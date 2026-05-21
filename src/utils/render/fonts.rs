//! Locate a bold sans-serif font under `C:\Windows\Fonts` and cache the loaded
//! glyph data.

use std::path::PathBuf;
use std::sync::OnceLock;

use ab_glyph::{FontVec, InvalidFont};

/// Font filenames probed in priority order.
const FONT_CANDIDATES: &[&str] = &[
    "ariblk.ttf",
    "arialbd.ttf",
    "segoeuib.ttf",
    "bahnschrift.ttf",
    "segoeui.ttf",
    "arial.ttf",
];

static FONT: OnceLock<FontVec> = OnceLock::new();

fn load_font() -> Result<FontVec, String> {
    let dir = PathBuf::from(r"C:\Windows\Fonts");
    for name in FONT_CANDIDATES {
        let path = dir.join(name);
        if path.is_file() {
            let bytes = std::fs::read(&path)
                .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
            return FontVec::try_from_vec(bytes)
                .map_err(|InvalidFont| format!("invalid font file: {}", path.display()));
        }
    }
    Err(format!(
        "no suitable bold sans-serif font found in {} (tried {:?})",
        dir.display(),
        FONT_CANDIDATES
    ))
}

/// Returns a static reference to a bold sans-serif font loaded from
/// `C:\Windows\Fonts`. Panics with a clear message if none of the candidate
/// fonts can be loaded.
pub fn font() -> &'static FontVec {
    FONT.get_or_init(|| load_font().expect("overlay font load failed"))
}
