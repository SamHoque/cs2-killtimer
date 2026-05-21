//! Rasterize timer digits via `ab_glyph` and emit premultiplied BGRA bytes
//! suitable for `UpdateLayeredWindow`. Adds a vertical gradient
//! (`top_rgb` → `bottom_rgb`), a dark outline (dilation around the glyph
//! silhouette), and a global `alpha` multiplier for fade-in transitions.

use ab_glyph::{Font, PxScale, ScaleFont};

use crate::core::settings::{
    OVERLAY_FONT_SIZE, OVERLAY_OUTLINE_RADIUS_PX, OVERLAY_OUTLINE_RGB,
};
use crate::utils::render::fonts::font;

/// Transparent border (in addition to the outline radius) around the text.
const PAD: u32 = 4;

/// A rasterized text bitmap ready to feed into `UpdateLayeredWindow`.
pub struct Glyph {
    pub w: u32,
    pub h: u32,
    /// `w * h * 4` bytes, BGRA, premultiplied alpha.
    pub bgra_premul: Vec<u8>,
}

pub fn rasterize(
    text: &str,
    top_rgb: (u8, u8, u8),
    bottom_rgb: (u8, u8, u8),
    alpha: f32,
) -> Glyph {
    let f = font();
    let scale = PxScale::from(OVERLAY_FONT_SIZE as f32);
    let scaled = f.as_scaled(scale);

    let mut pen_x: f32 = 0.0;
    let mut last_id: Option<ab_glyph::GlyphId> = None;
    let mut outlined: Vec<ab_glyph::OutlinedGlyph> = Vec::with_capacity(text.len());
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    );

    for ch in text.chars() {
        let id = scaled.glyph_id(ch);
        if let Some(prev) = last_id {
            pen_x += scaled.kern(prev, id);
        }
        let glyph = id.with_scale_and_position(scale, ab_glyph::point(pen_x, 0.0));
        if let Some(og) = f.outline_glyph(glyph) {
            let b = og.px_bounds();
            min_x = min_x.min(b.min.x);
            min_y = min_y.min(b.min.y);
            max_x = max_x.max(b.max.x);
            max_y = max_y.max(b.max.y);
            outlined.push(og);
        }
        pen_x += scaled.h_advance(id);
        last_id = Some(id);
    }

    let (text_w, text_h) = if outlined.is_empty() {
        (1, 1)
    } else {
        (
            (max_x - min_x).ceil().max(1.0) as u32,
            (max_y - min_y).ceil().max(1.0) as u32,
        )
    };
    let pad = PAD + OVERLAY_OUTLINE_RADIUS_PX;
    let w = text_w + pad * 2;
    let h = text_h + pad * 2;

    // ---- 1. Build glyph coverage map (one u8 per pixel). --------------------
    let mut cov = vec![0u8; (w * h) as usize];
    for og in &outlined {
        let bounds = og.px_bounds();
        let off_x = bounds.min.x - min_x + pad as f32;
        let off_y = bounds.min.y - min_y + pad as f32;
        og.draw(|gx, gy, coverage| {
            let px = off_x as i32 + gx as i32;
            let py = off_y as i32 + gy as i32;
            if px < 0 || py < 0 || (px as u32) >= w || (py as u32) >= h {
                return;
            }
            let a = (coverage.clamp(0.0, 1.0) * 255.0) as u8;
            let idx = (py as u32 * w + px as u32) as usize;
            if a > cov[idx] {
                cov[idx] = a;
            }
        });
    }

    // ---- 2. Compose BGRA with outline (dilated silhouette) + gradient fill. -
    let mut bgra = vec![0u8; (w * h * 4) as usize];
    let alpha = alpha.clamp(0.0, 1.0);
    let denom = (text_h as f32 - 1.0).max(1.0);
    let r = OVERLAY_OUTLINE_RADIUS_PX as i32;
    let r2 = r * r;
    let (or_, og_, ob_) = OVERLAY_OUTLINE_RGB;
    let outline = (or_ as u32, og_ as u32, ob_ as u32);

    for py in 0..h as i32 {
        for px in 0..w as i32 {
            let center = cov[(py as u32 * w + px as u32) as usize];

            let (color, base_alpha) = if center > 0 {
                // Fill pixel — vertical gradient between top_rgb and bottom_rgb.
                let t = ((py as u32).saturating_sub(pad).min(text_h.saturating_sub(1))) as f32
                    / denom;
                let rr = (top_rgb.0 as f32 * (1.0 - t) + bottom_rgb.0 as f32 * t) as u32;
                let gg = (top_rgb.1 as f32 * (1.0 - t) + bottom_rgb.1 as f32 * t) as u32;
                let bb = (top_rgb.2 as f32 * (1.0 - t) + bottom_rgb.2 as f32 * t) as u32;
                ((rr, gg, bb), center)
            } else {
                // Outline pixel — pick max coverage in a circular neighborhood.
                let mut max_n = 0u8;
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx * dx + dy * dy > r2 {
                            continue;
                        }
                        let nx = px + dx;
                        let ny = py + dy;
                        if nx < 0 || ny < 0 || (nx as u32) >= w || (ny as u32) >= h {
                            continue;
                        }
                        let nc = cov[(ny as u32 * w + nx as u32) as usize];
                        if nc > max_n {
                            max_n = nc;
                        }
                    }
                }
                if max_n == 0 {
                    continue;
                }
                (outline, max_n)
            };

            let a = ((base_alpha as f32 * alpha) as u32).min(255);
            if a == 0 {
                continue;
            }
            let idx = ((py as u32 * w + px as u32) * 4) as usize;
            // Premultiplied BGRA.
            bgra[idx] = (color.2 * a / 255) as u8;
            bgra[idx + 1] = (color.1 * a / 255) as u8;
            bgra[idx + 2] = (color.0 * a / 255) as u8;
            bgra[idx + 3] = a as u8;
        }
    }

    Glyph { w, h, bgra_premul: bgra }
}
