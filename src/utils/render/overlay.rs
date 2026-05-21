//! A transparent topmost popup that draws the timer over CS2, plus the
//! foreground check that hides it when CS2 isn't focused.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{Result, anyhow};
use windows::Win32::Foundation::{CloseHandle, HWND, POINT, RECT, SIZE};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, ClientToScreen,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HBITMAP,
    HGDIOBJ, ReleaseDC, SelectObject,
};
use windows::Win32::System::ProcessStatus::GetModuleFileNameExW;
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, GetClientRect, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    SW_HIDE, SW_SHOWNOACTIVATE, ShowWindow, ULW_ALPHA, UpdateLayeredWindow, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::PCWSTR;

use crate::common::TARGET_WINDOW_TITLE;
use crate::core::settings::{
    OVERLAY_FADE_MS, OVERLAY_GRADIENT_BOTTOM_DARKEN, OVERLAY_GRADIENT_TOP_LIGHTEN, OVERLAY_TICK_MS,
};
use crate::core::state::SharedState;
use crate::utils::render::text::{Glyph, rasterize};

/// Lower-case basename of the exe for the given pid, or empty on failure.
fn process_exe_basename(pid: u32) -> String {
    if pid == 0 {
        return String::new();
    }
    let h = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) } {
        Ok(h) => h,
        Err(_) => return String::new(),
    };
    let mut buf = [0u16; 4096];
    let len = unsafe { GetModuleFileNameExW(Some(h), None, &mut buf) } as usize;
    unsafe {
        let _ = CloseHandle(h);
    }
    if len == 0 {
        return String::new();
    }
    let path = String::from_utf16_lossy(&buf[..len]).replace('/', "\\");
    path.rsplit('\\').next().unwrap_or("").to_string()
}

/// Returns the foreground HWND when it belongs to cs2.exe, or its title starts
/// with `"Counter-Strike 2"`. None otherwise.
fn cs2_foreground_hwnd() -> Option<HWND> {
    let fg = unsafe { GetForegroundWindow() };
    if fg.is_invalid() {
        return None;
    }
    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(fg, Some(&mut pid)) };
    if process_exe_basename(pid).eq_ignore_ascii_case("cs2.exe") {
        return Some(fg);
    }
    let mut tbuf = [0u16; 512];
    let n = unsafe { GetWindowTextW(fg, &mut tbuf) } as usize;
    let title = String::from_utf16_lossy(&tbuf[..n]);
    if title.trim().starts_with(TARGET_WINDOW_TITLE) {
        Some(fg)
    } else {
        None
    }
}

/// Client-area rect of `hwnd` in screen coordinates, as `(x, y, w, h)`. This is
/// what we anchor the overlay to so it tracks CS2 in windowed mode and lines up
/// with the game's render surface in borderless / fullscreen.
fn client_rect_screen(hwnd: HWND) -> Option<(i32, i32, i32, i32)> {
    unsafe {
        let mut rc = RECT::default();
        GetClientRect(hwnd, &mut rc).ok()?;
        let mut tl = POINT { x: 0, y: 0 };
        if !ClientToScreen(hwnd, &mut tl).as_bool() {
            return None;
        }
        let w = rc.right - rc.left;
        let h = rc.bottom - rc.top;
        if w <= 0 || h <= 0 {
            return None;
        }
        Some((tl.x, tl.y, w, h))
    }
}

struct LayeredWindow {
    hwnd: HWND,
}

impl LayeredWindow {
    fn create() -> Result<Self> {
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE,
                PCWSTR(wide_str("STATIC").as_ptr()),
                PCWSTR(wide_str("").as_ptr()),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                None,
                None,
            )
        }
        .map_err(|e| anyhow!("CreateWindowExW: {e}"))?;
        Ok(Self { hwnd })
    }

    fn hide(&mut self) {
        if !self.hwnd.is_invalid() {
            let _ = unsafe { ShowWindow(self.hwnd, SW_HIDE) };
        }
    }

    fn update(&mut self, g: &Glyph, x: i32, y: i32) {
        let w = g.w as i32;
        let h = g.h as i32;
        if w <= 0 || h <= 0 {
            return;
        }

        unsafe {
            let screen_dc = GetDC(None);
            let mem_dc = CreateCompatibleDC(Some(screen_dc));

            let mut bmi = BITMAPINFO::default();
            bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = w;
            bmi.bmiHeader.biHeight = -h; // top-down
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB.0;

            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let hbitmap: HBITMAP = match CreateDIBSection(
                Some(screen_dc),
                &bmi,
                DIB_RGB_COLORS,
                &mut bits,
                None,
                0,
            ) {
                Ok(h) => h,
                Err(_) => {
                    let _ = DeleteDC(mem_dc);
                    ReleaseDC(None, screen_dc);
                    return;
                }
            };
            let old_obj: HGDIOBJ = SelectObject(mem_dc, hbitmap.into());

            if !bits.is_null() {
                let len = g.bgra_premul.len();
                let need = (w as usize) * (h as usize) * 4;
                let copy_len = len.min(need);
                std::ptr::copy_nonoverlapping(g.bgra_premul.as_ptr(), bits as *mut u8, copy_len);
            }

            let dst = POINT { x, y };
            let size = SIZE { cx: w, cy: h };
            let src = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let _ = UpdateLayeredWindow(
                self.hwnd,
                Some(screen_dc),
                Some(&dst),
                Some(&size),
                Some(mem_dc),
                Some(&src),
                Default::default(),
                Some(&blend),
                ULW_ALPHA,
            );
            let _ = ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);

            SelectObject(mem_dc, old_obj);
            let _ = DeleteObject(hbitmap.into());
            let _ = DeleteDC(mem_dc);
            ReleaseDC(None, screen_dc);
        }
    }
}

/// Lerp each channel toward white by `t` ∈ [0, 1].
fn lighten(rgb: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    let l = |c: u8| (c as f32 + (255.0 - c as f32) * t).clamp(0.0, 255.0) as u8;
    (l(rgb.0), l(rgb.1), l(rgb.2))
}

/// Lerp each channel toward black by `t` ∈ [0, 1].
fn darken(rgb: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let t = 1.0 - t.clamp(0.0, 1.0);
    let d = |c: u8| (c as f32 * t).clamp(0.0, 255.0) as u8;
    (d(rgb.0), d(rgb.1), d(rgb.2))
}

/// UTF-16, null-terminated. Caller must hold the returned `Vec` alive for the
/// duration of the FFI call.
fn wide_str(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Blocking overlay event loop. Owns the layered window for the process
/// lifetime; the caller must run this on the main thread.
pub fn run(state: Arc<SharedState>) -> Result<()> {
    let mut window = LayeredWindow::create()?;
    let tick = Duration::from_millis(OVERLAY_TICK_MS);
    let fade = Duration::from_millis(OVERLAY_FADE_MS);
    let mut last_text = String::new();
    let mut fade_start: Option<std::time::Instant> = None;

    loop {
        thread::sleep(tick);

        let (visible, value, rgb, connected) = state.snapshot();
        let cs2 = cs2_foreground_hwnd();
        if !visible || !connected || cs2.is_none() {
            window.hide();
            last_text.clear();
            fade_start = None;
            continue;
        }
        let Some((cx, cy, cw, ch)) = client_rect_screen(cs2.unwrap()) else {
            window.hide();
            last_text.clear();
            fade_start = None;
            continue;
        };

        let text = value.to_string();
        if text != last_text {
            fade_start = Some(std::time::Instant::now());
            last_text = text.clone();
        }

        let alpha = match fade_start {
            Some(t) => {
                let e = t.elapsed().as_secs_f32();
                (e / fade.as_secs_f32()).clamp(0.0, 1.0)
            }
            None => 1.0,
        };

        let top = lighten(rgb, OVERLAY_GRADIENT_TOP_LIGHTEN);
        let bottom = darken(rgb, OVERLAY_GRADIENT_BOTTOM_DARKEN);
        let g = rasterize(&text, top, bottom, alpha);
        let x = cx + cw / 2 - (g.w as i32) / 2;
        let y = cy + ch / 2 + 100 - (g.h as i32) / 2;
        window.update(&g, x, y);
    }
}
