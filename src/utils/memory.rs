//! Process attach, module enumeration, and a copyable `Handle` for chained
//! reads in the target process.

use std::mem::{MaybeUninit, size_of};

use anyhow::{Result, anyhow};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, MODULEENTRY32W, Module32FirstW, Module32NextW, PROCESSENTRY32W,
    Process32FirstW, Process32NextW, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};

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
