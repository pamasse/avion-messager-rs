#![allow(dead_code)] // câblé en Task 16/18

use ab_glyph::{Font, FontVec, PxScale, ScaleFont};

const C_RED: u32 = 0xe23b3b;
const C_RED_D: u32 = 0xc62f2f;
const C_RED_DD: u32 = 0xa81f1f;
const C_PINK: u32 = 0xf7a9a9;
const C_GREY: u32 = 0xc9d4e0;
const C_GREY_L: u32 = 0xe6edf4;
const C_GLASS: u32 = 0xbfe3ff;
const C_DARK: u32 = 0x3b3f47;
const C_DARKER: u32 = 0x2b2f36;
const C_HUB: u32 = 0xffcf3f;
const C_WHITE: u32 = 0xffffff;

const BANNER_RECTS: &[(i32, i32, i32, i32, u32)] = &[
    (40, 86, 10, 28, C_RED_DD),
    (50, 78, 10, 44, C_RED_D),
    (60, 72, 500, 56, C_RED),
    (60, 72, 500, 6, C_PINK),
    (60, 122, 500, 6, C_RED_DD),
];
// avion : offset (690, 52) déjà appliqué ci-dessous
const PLANE_RECTS: &[(i32, i32, i32, i32, u32)] = &[
    (716, 60, 16, 34, C_GREY), (716, 60, 16, 6, C_GREY_L),         // dérive
    (730, 82, 92, 10, C_RED), (720, 92, 112, 10, C_RED),           // fuselage haut
    (720, 102, 112, 10, C_RED_D), (730, 112, 92, 10, C_RED_D),     // fuselage bas
    (786, 84, 14, 8, C_GLASS), (804, 84, 14, 8, C_GLASS),          // hublots
    (738, 122, 72, 10, C_GREY), (738, 122, 72, 4, C_GREY_L),       // aile
    (754, 132, 4, 12, C_DARK), (794, 132, 4, 12, C_DARK),          // jambes de train
    (746, 144, 20, 8, C_DARKER), (786, 144, 20, 8, C_DARKER),      // roues
    (832, 92, 12, 20, C_DARKER), (840, 94, 10, 10, C_HUB),         // nez + moyeu
    (850, 74, 6, 56, C_DARK),                                       // hélice
];

/// Câble pointillé (ligne (560,96)→(700,92), épaisseur 5, tirets 3/7) — approximation en rects.
fn tow_line_rects() -> Vec<(i32, i32, i32, i32, u32)> {
    // 14 pas de 10 px le long de x, y interpolé linéairement de 96 à 92
    (0..14)
        .map(|i| {
            let x = 560 + i * 10;
            let y = 96 - (i * 4) / 14; // 96 → 92
            (x, y - 2, 3, 5, C_DARK)
        })
        .collect()
}

pub struct Bitmap {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u8>, // BGRA prémultiplié, top-down
}

impl Bitmap {
    fn new(w: usize, h: usize) -> Bitmap {
        Bitmap { w, h, px: vec![0; w * h * 4] }
    }

    fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, rgb: u32) {
        let (b, g, r) = ((rgb & 0xff) as u8, ((rgb >> 8) & 0xff) as u8, ((rgb >> 16) & 0xff) as u8);
        for yy in y.max(0)..(y + h).min(self.h as i32) {
            for xx in x.max(0)..(x + w).min(self.w as i32) {
                let i = (yy as usize * self.w + xx as usize) * 4;
                self.px[i..i + 4].copy_from_slice(&[b, g, r, 0xff]);
            }
        }
    }
}

fn load_font() -> Option<FontVec> {
    for name in ["consolab.ttf", "courbd.ttf", "cour.ttf"] {
        if let Ok(bytes) = std::fs::read(format!("C:\\Windows\\Fonts\\{name}")) {
            if let Ok(f) = FontVec::try_from_vec(bytes) {
                return Some(f);
            }
        }
    }
    None
}

/// Texte pixel-art : coverage seuillée (>= 0.5 → blanc opaque), centré sur (cx, baseline).
fn draw_text(b: &mut Bitmap, text: &str, cx: f32, baseline: f32, size: f32) {
    let Some(font) = load_font() else { return };
    let scaled = font.as_scaled(PxScale::from(size));
    let width: f32 = text.chars().map(|c| scaled.h_advance(scaled.glyph_id(c))).sum();
    let mut pen_x = cx - width / 2.0;
    for c in text.chars() {
        let glyph_id = scaled.glyph_id(c);
        let glyph = glyph_id.with_scale_and_position(PxScale::from(size), ab_glyph::point(pen_x, baseline));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, cov| {
                if cov >= 0.5 {
                    let x = bounds.min.x as i32 + gx as i32;
                    let y = bounds.min.y as i32 + gy as i32;
                    if x >= 0 && y >= 0 && (x as usize) < b.w && (y as usize) < b.h {
                        let i = (y as usize * b.w + x as usize) * 4;
                        b.px[i..i + 4].copy_from_slice(&[0xff, 0xff, 0xff, 0xff]);
                    }
                }
            });
        }
        pen_x += scaled.h_advance(glyph_id);
    }
}

fn scale_nearest(src: &Bitmap, scale: f32) -> Bitmap {
    let (w, h) = ((src.w as f32 * scale) as usize, (src.h as f32 * scale) as usize);
    let mut dst = Bitmap::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let sx = ((x as f32 / scale) as usize).min(src.w - 1);
            let sy = ((y as f32 / scale) as usize).min(src.h - 1);
            let (si, di) = ((sy * src.w + sx) * 4, (y * w + x) * 4);
            let pixel: [u8; 4] = src.px[si..si + 4].try_into().unwrap();
            dst.px[di..di + 4].copy_from_slice(&pixel);
        }
    }
    dst
}

/// Rig complet (drap + avion + câble + texte), mis à l'échelle. Dimensions : (900·scale)×(200·scale).
pub fn render_rig(text: &str, scale: f32) -> Bitmap {
    let mut b = Bitmap::new(900, 200);
    for &(x, y, w, h, c) in BANNER_RECTS {
        b.fill_rect(x, y, w, h, c);
    }
    for (x, y, w, h, c) in tow_line_rects() {
        b.fill_rect(x, y, w, h, c);
    }
    for &(x, y, w, h, c) in PLANE_RECTS {
        b.fill_rect(x, y, w, h, c);
    }
    draw_text(&mut b, text, 310.0, 109.0, 26.0);
    if (scale - 1.0).abs() < f32::EPSILON {
        b
    } else {
        scale_nearest(&b, scale)
    }
}

/// Icône 32×32 pour le tray : avion seul, remappé depuis PLANE_RECTS.
pub fn render_icon() -> Bitmap {
    let mut b = Bitmap::new(32, 32);
    // avion remappé : la zone (716..856, 60..152) → 32×32 (÷ ~4.6, offset)
    for &(x, y, w, h, c) in PLANE_RECTS {
        let sx = ((x - 716) as f32 * 32.0 / 140.0) as i32;
        let sy = ((y - 60) as f32 * 32.0 / 140.0) as i32 + 5;
        let sw = ((w as f32 * 32.0 / 140.0) as i32).max(1);
        let sh = ((h as f32 * 32.0 / 140.0) as i32).max(1);
        b.fill_rect(sx, sy, sw, sh, c);
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(b: &Bitmap, x: usize, y: usize) -> (u8, u8, u8, u8) {
        let i = (y * b.w + x) * 4;
        (b.px[i], b.px[i + 1], b.px[i + 2], b.px[i + 3]) // B,G,R,A
    }

    #[test]
    fn dimensions_a_l_echelle() {
        let b = render_rig("Test", 1.0);
        assert_eq!((b.w, b.h), (900, 200));
        let b2 = render_rig("Test", 2.0);
        assert_eq!((b2.w, b2.h), (1800, 400));
    }

    #[test]
    fn drap_rouge_et_fond_transparent() {
        let b = render_rig("", 1.0);
        // centre du drap sans texte : #e23b3b opaque → BGRA (0x3b, 0x3b, 0xe2, 0xff)
        assert_eq!(px(&b, 100, 100), (0x3b, 0x3b, 0xe2, 0xff));
        // hors rig : alpha 0
        assert_eq!(px(&b, 10, 10).3, 0);
    }

    #[test]
    fn premultiplie_canaux_jamais_superieurs_a_alpha() {
        let b = render_rig("09 h 05 — Point produit", 1.0);
        for p in b.px.chunks_exact(4) {
            assert!(p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3]);
        }
    }

    #[test]
    fn le_texte_marque_des_pixels_blancs_dans_le_drap() {
        let vide = render_rig("", 1.0);
        let avec = render_rig("RÉUNION", 1.0);
        assert_ne!(vide.px, avec.px);
        // au moins un pixel blanc opaque dans la zone du drap (y 78..122)
        let mut found = false;
        for y in 78..122 {
            for x in 60..560 {
                let i = (y * avec.w + x) * 4;
                if avec.px[i..i + 4] == [0xff, 0xff, 0xff, 0xff] {
                    found = true;
                }
            }
        }
        assert!(found);
    }

    #[test]
    fn icone_32x32_non_vide() {
        let icon = render_icon();
        assert_eq!((icon.w, icon.h), (32, 32));
        assert!(icon.px.chunks_exact(4).any(|p| p[3] > 0));
    }
}
