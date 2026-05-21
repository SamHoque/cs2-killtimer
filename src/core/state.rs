//! Snapshot of overlay state shared between the kill-timer worker and the
//! overlay loop. One Mutex, four fields.

use std::sync::Mutex;

#[derive(Clone, Copy)]
struct Inner {
    visible: bool,
    value: u32,
    rgb: (u8, u8, u8),
    connected: bool,
}

const INITIAL: Inner = Inner {
    visible: false,
    value: 1,
    rgb: crate::core::settings::OVERLAY_HIDDEN_RGB,
    connected: false,
};

pub struct SharedState {
    inner: Mutex<Inner>,
}

impl SharedState {
    pub const fn new() -> Self {
        Self { inner: Mutex::new(INITIAL) }
    }

    pub fn set(&self, visible: bool, value: u32, rgb: (u8, u8, u8), connected: bool) {
        if let Ok(mut g) = self.inner.lock() {
            *g = Inner { visible, value: value.max(1), rgb, connected };
        }
    }

    /// `(visible, value, rgb, connected)`.
    pub fn snapshot(&self) -> (bool, u32, (u8, u8, u8), bool) {
        let i = self.inner.lock().map(|g| *g).unwrap_or(INITIAL);
        (i.visible, i.value, i.rgb, i.connected)
    }
}

impl Default for SharedState {
    fn default() -> Self {
        Self::new()
    }
}
