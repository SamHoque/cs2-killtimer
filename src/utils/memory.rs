//! Process attach, module enumeration, and a copyable `Handle` for chained
//! reads in the target process.

use std::ffi::{CString, c_void};
use std::mem::{MaybeUninit, size_of};

use anyhow::{Result, anyhow};
use windows::Win32::Foundation::{
    CloseHandle, DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE,
};
use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, MODULEENTRY32W, Module32FirstW, Module32NextW, PROCESSENTRY32W,
    Process32FirstW, Process32NextW, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
use windows::Win32::System::Memory::{
    MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_GUARD, PAGE_NOACCESS, VirtualQueryEx,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetProcessId, OpenProcess, PROCESS_DUP_HANDLE,
    PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowA, GetWindowThreadProcessId};
use windows::core::PCSTR;

struct Raw(HANDLE);

impl Drop for Raw {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

pub struct Process {
    h: Raw,
    pub pid: u32,
}

// HANDLE is just a kernel-object id; ReadProcessMemory is thread-safe.
unsafe impl Send for Process {}
unsafe impl Sync for Process {}

impl Process {
    /// Open the first running process whose exe basename matches (case-insensitive).
    pub fn attach(exe: &str) -> Result<Self> {
        let pid = find_pid(exe)?.ok_or_else(|| anyhow!("process `{exe}` not found"))?;
        let raw = unsafe {
            OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_INFORMATION, false, pid)
        }
        .map_err(|e| anyhow!("OpenProcess({pid}): {e}"))?;
        Ok(Self { h: Raw(raw), pid })
    }

    /// True if any process with this exe basename is running.
    pub fn running(exe: &str) -> bool {
        matches!(find_pid(exe), Ok(Some(_)))
    }

    /// Wrap a raw address into a `handle` bound to this process.
    pub fn at(&self, addr: u64) -> Handle<'_> {
        Handle { proc: self, addr }
    }

    /// Resolve a loaded module's base address. `Handle::is_null()` will never be true on success.
    pub fn module(&self, name: &str) -> Result<Handle<'_>> {
        let snap = Raw(unsafe {
            CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, self.pid)
        }?);
        let mut e = MODULEENTRY32W {
            dwSize: size_of::<MODULEENTRY32W>() as u32,
            ..Default::default()
        };
        unsafe { Module32FirstW(snap.0, &mut e) }?;
        loop {
            if wide_eq_ascii(&e.szModule, name) {
                return Ok(self.at(e.modBaseAddr as u64));
            }
            if unsafe { Module32NextW(snap.0, &mut e) }.is_err() {
                return Err(anyhow!("module `{name}` not loaded in pid {}", self.pid));
            }
        }
    }

    /// Internal: read exactly `size_of::<T>()` bytes at `addr`. None on any failure
    /// (null address, unmapped page, short read, process gone).
    fn read_at<T: Copy>(&self, addr: u64) -> Option<T> {
        if addr == 0 {
            return None;
        }
        let mut out = MaybeUninit::<T>::uninit();
        let mut got = 0usize;
        let ok = unsafe {
            ReadProcessMemory(
                self.h.0,
                addr as *const _,
                out.as_mut_ptr().cast(),
                size_of::<T>(),
                Some(&mut got),
            )
        }
        .is_ok();
        (ok && got == size_of::<T>()).then(|| unsafe { out.assume_init() })
    }
}

#[derive(Copy, Clone)]
pub struct Handle<'p> {
    proc: &'p Process,
    addr: u64,
}

impl<'p> Handle<'p> {
    /// Raw 64-bit address.
    pub fn addr(self) -> u64 {
        self.addr
    }

    pub fn is_null(self) -> bool {
        self.addr == 0
    }

    /// `self + offset`, wrapping. Offsets from `Offsets` are `i64`; literals coerce.
    pub fn add(self, offset: impl Into<i64>) -> Self {
        Self {
            addr: self.addr.wrapping_add_signed(offset.into()),
            ..self
        }
    }

    /// `self - offset`, wrapping.
    pub fn sub(self, offset: impl Into<i64>) -> Self {
        self.add(offset.into().wrapping_neg())
    }

    /// Read a `T` at this address. None on failure or null address.
    pub fn read<T: Copy>(self) -> Option<T> {
        self.proc.read_at(self.addr)
    }

    /// Read a `T` or fall back to `T::default()` (zero for primitives) — convenience
    /// for the swallow-and-continue style of the original Python worker.
    pub fn read_or<T: Copy + Default>(self) -> T {
        self.read().unwrap_or_default()
    }

    /// Dereference a 64-bit pointer at this address; null handle on failure.
    /// Lets you chain: `proc.at(pawn).add(off.x).deref().add(off.y).read::<u32>()`.
    pub fn deref(self) -> Self {
        Self {
            proc: self.proc,
            addr: self.read_or::<u64>(),
        }
    }

    /// RIP-relative resolve: `self + 4 + *(i32*)self`. For x86-64 sig-scan results.
    pub fn rip(self) -> Self {
        let disp = self.read_or::<i32>() as i64;
        self.add(4_i64).add(disp)
    }
}

fn find_pid(exe: &str) -> Result<Option<u32>> {
    let snap = Raw(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }?);
    let mut e = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    if unsafe { Process32FirstW(snap.0, &mut e) }.is_err() {
        return Ok(None);
    }
    loop {
        if wide_eq_ascii(&e.szExeFile, exe) {
            return Ok(Some(e.th32ProcessID));
        }
        if unsafe { Process32NextW(snap.0, &mut e) }.is_err() {
            return Ok(None);
        }
    }
}

fn wide_eq_ascii(wide: &[u16], target: &str) -> bool {
    let len = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    String::from_utf16_lossy(&wide[..len]).eq_ignore_ascii_case(target)
}

// ---- handle-hijack helper ---------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone)]
struct SystemHandleTableEntryInfo {
    unique_process_id: u16,
    creator_back_trace_index: u16,
    object_type_index: u8,
    handle_attributes: u8,
    handle_value: u16,
    object: *mut c_void,
    granted_access: u32,
}

#[repr(C)]
struct SystemHandleInformationHeader {
    count: u32,
    // followed by `count` SystemHandleTableEntryInfo entries
}

type PfnNtQuerySystemInformation = unsafe extern "system" fn(
    system_information_class: u32,
    system_information: *mut c_void,
    system_information_length: u32,
    return_length: *mut u32,
) -> i32;

const SYSTEM_HANDLE_INFORMATION_CLASS: u32 = 0x10;
const STATUS_INFO_LENGTH_MISMATCH: u32 = 0xC000_0004;

/// Result of [`find_process_with_handle`].
pub struct HijackedHandle {
    /// Handle to the target process, duplicated into the calling process.
    pub handle: HANDLE,
    /// PID of the process that originally held the donor handle.
    pub source_pid: u32,
    /// Handle value as seen in the donor process's handle table.
    pub original_handle_value: u16,
    /// PID of the target process resolved from the window title.
    pub target_pid: u32,
}

/// Locate a process via its top-level window title, then scan the system handle
/// table for an existing handle to that process held by some other process and
/// duplicate it into ours. Useful when calling `OpenProcess` on the target is
/// blocked but another process already has a suitable handle open.
///
/// `access_mask` filters donor handles: only entries whose `GrantedAccess`
/// already covers the requested bits are considered, so the duplicate inherits
/// them via `DUPLICATE_SAME_ACCESS`.
pub fn find_process_with_handle(
    window_name: &str,
    access_mask: u32,
) -> Result<HijackedHandle> {
    let target_pid = target_pid_from_window(window_name)?;
    let nt_query = nt_query_system_information()?;

    // The table can be large on busy systems; grow on STATUS_INFO_LENGTH_MISMATCH.
    let mut buffer: Vec<u8> = vec![0u8; 1 << 20];
    loop {
        let mut needed: u32 = 0;
        let status = unsafe {
            nt_query(
                SYSTEM_HANDLE_INFORMATION_CLASS,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                &mut needed,
            )
        };
        if status as u32 == STATUS_INFO_LENGTH_MISMATCH {
            let new_len = (buffer.len() * 2).max(needed as usize + 4096);
            buffer.resize(new_len, 0);
            continue;
        }
        if status != 0 {
            return Err(anyhow!(
                "NtQuerySystemInformation failed: 0x{:08x}",
                status as u32
            ));
        }
        break;
    }

    let header = unsafe { &*(buffer.as_ptr() as *const SystemHandleInformationHeader) };
    let entries_ptr = unsafe {
        buffer
            .as_ptr()
            .add(size_of::<SystemHandleInformationHeader>())
            as *const SystemHandleTableEntryInfo
    };
    let entries = unsafe { std::slice::from_raw_parts(entries_ptr, header.count as usize) };

    for entry in entries {
        if entry.granted_access & access_mask != access_mask {
            continue;
        }
        let donor_pid = entry.unique_process_id as u32;
        let donor = match unsafe { OpenProcess(PROCESS_DUP_HANDLE, false, donor_pid) } {
            Ok(h) => h,
            Err(_) => continue,
        };
        let src = HANDLE(entry.handle_value as usize as *mut c_void);
        if let Some(dup) = dup_if_target(donor, src, target_pid) {
            unsafe { let _ = CloseHandle(donor); }
            return Ok(HijackedHandle {
                handle: dup,
                source_pid: donor_pid,
                original_handle_value: entry.handle_value,
                target_pid,
            });
        }
        unsafe { let _ = CloseHandle(donor); }
    }

    Err(anyhow!(
        "no donor handle with access 0x{access_mask:x} found for pid {target_pid}"
    ))
}

/// Walk every committed, readable region of `donor_pid`'s address space looking
/// for a pointer-sized slot whose value equals `handle` and which, when
/// duplicated, resolves to the process owning `window_name`. Returns the
/// address inside the donor where that slot lives.
///
/// Slow on large donors: it does `ReadProcessMemory` over the donor's full
/// committed working set. Pair with [`find_process_with_handle`] to learn
/// `handle` + `donor_pid` first.
pub fn find_address_with_handle(
    handle: HANDLE,
    donor_pid: u32,
    window_name: &str,
) -> Result<usize> {
    let target_pid = target_pid_from_window(window_name)?;
    let donor = unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | PROCESS_DUP_HANDLE,
            false,
            donor_pid,
        )
    }
    .map_err(|e| anyhow!("OpenProcess({donor_pid}): {e}"))?;

    let needle = handle.0 as usize;
    let bad_protect = PAGE_GUARD.0 | PAGE_NOACCESS.0;
    let mut buffer: Vec<u8> = Vec::new();
    let mut address: usize = 0;

    loop {
        let mut mbi = MaybeUninit::<MEMORY_BASIC_INFORMATION>::uninit();
        let got = unsafe {
            VirtualQueryEx(
                donor,
                Some(address as *const c_void),
                mbi.as_mut_ptr(),
                size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        };
        if got == 0 {
            break;
        }
        let mbi = unsafe { mbi.assume_init() };
        let region_size = mbi.RegionSize;

        if mbi.State == MEM_COMMIT && (mbi.Protect.0 & bad_protect) == 0 {
            if buffer.len() < region_size {
                buffer.resize(region_size, 0);
            }
            let mut bytes_read: usize = 0;
            let ok = unsafe {
                ReadProcessMemory(
                    donor,
                    address as *const c_void,
                    buffer.as_mut_ptr().cast(),
                    region_size,
                    Some(&mut bytes_read),
                )
            }
            .is_ok();
            if ok {
                let words = bytes_read / size_of::<usize>();
                let slice = unsafe {
                    std::slice::from_raw_parts(buffer.as_ptr() as *const usize, words)
                };
                for (i, &word) in slice.iter().enumerate() {
                    if word != needle {
                        continue;
                    }
                    let candidate = HANDLE(word as *mut c_void);
                    if let Some(dup) = dup_if_target(donor, candidate, target_pid) {
                        unsafe {
                            let _ = CloseHandle(dup);
                            let _ = CloseHandle(donor);
                        }
                        return Ok(address + i * size_of::<usize>());
                    }
                }
            }
        }

        let next = address.wrapping_add(region_size);
        if next <= address {
            break;
        }
        address = next;
    }

    unsafe { let _ = CloseHandle(donor); }
    Err(anyhow!(
        "handle 0x{needle:x} not found in pid {donor_pid} memory"
    ))
}

// ---- shared helpers --------------------------------------------------------

fn target_pid_from_window(window_name: &str) -> Result<u32> {
    let cwindow = CString::new(window_name)
        .map_err(|_| anyhow!("window name contains a NUL byte"))?;
    let hwnd = unsafe { FindWindowA(PCSTR::null(), PCSTR(cwindow.as_ptr() as *const u8)) }
        .map_err(|e| anyhow!("FindWindow({window_name}): {e}"))?;
    if hwnd.0.is_null() {
        return Err(anyhow!("window `{window_name}` not found"));
    }
    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return Err(anyhow!("GetWindowThreadProcessId returned 0"));
    }
    Ok(pid)
}

/// Duplicate `src_handle` from `src_proc` into our process. Returns the dup if
/// it resolves to `expected_pid`; otherwise closes it and returns `None`.
fn dup_if_target(src_proc: HANDLE, src_handle: HANDLE, expected_pid: u32) -> Option<HANDLE> {
    let mut dup = HANDLE::default();
    let ok = unsafe {
        DuplicateHandle(
            src_proc,
            src_handle,
            GetCurrentProcess(),
            &mut dup,
            0,
            false,
            DUPLICATE_SAME_ACCESS,
        )
    }
    .is_ok();
    if !ok {
        return None;
    }
    if unsafe { GetProcessId(dup) } == expected_pid {
        Some(dup)
    } else {
        unsafe { let _ = CloseHandle(dup); }
        None
    }
}

fn nt_query_system_information() -> Result<PfnNtQuerySystemInformation> {
    let ntdll = unsafe { GetModuleHandleA(PCSTR(b"ntdll.dll\0".as_ptr())) }
        .map_err(|e| anyhow!("GetModuleHandle(ntdll): {e}"))?;
    let pfn = unsafe { GetProcAddress(ntdll, PCSTR(b"NtQuerySystemInformation\0".as_ptr())) }
        .ok_or_else(|| anyhow!("NtQuerySystemInformation not exported by ntdll"))?;
    Ok(unsafe { std::mem::transmute(pfn) })
}
