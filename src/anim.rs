#![allow(dead_code)] // câblé en Task 16

pub const RIG_W: i32 = 900;
pub const RIG_H: i32 = 200;
pub const FLIGHT_MS: u32 = 12_000;
const BOB_PERIOD_MS: f32 = 2_500.0;
const BOB_AMPLITUDE: f32 = 8.0;
const START_X: f32 = -950.0;

pub fn position(t_ms: u32, screen_w: i32, screen_h: i32, scale: f32) -> (i32, i32) {
    let t = t_ms.min(FLIGHT_MS) as f32;
    let progress = t / FLIGHT_MS as f32;
    let x = START_X * scale + (screen_w as f32 - START_X * scale) * progress;
    let bob = -BOB_AMPLITUDE * scale * (1.0 - (std::f32::consts::TAU * t / BOB_PERIOD_MS).cos()) / 2.0;
    let y = 0.12 * screen_h as f32 + bob;
    (x as i32, y as i32)
}

pub fn finished(t_ms: u32) -> bool {
    t_ms >= FLIGHT_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depart_hors_ecran_gauche_arrivee_hors_ecran_droite() {
        let (x0, y0) = position(0, 1920, 1080, 1.0);
        assert_eq!(x0, -950);
        assert_eq!(y0, (0.12 * 1080.0) as i32); // bob(0) = 0
        let (x1, _) = position(FLIGHT_MS, 1920, 1080, 1.0);
        assert_eq!(x1, 1920);
    }

    #[test]
    fn traversee_lineaire_mi_parcours() {
        // x(6 s) = −950 + (1920 − (−950)) × 0,5 = 485
        let (x, _) = position(FLIGHT_MS / 2, 1920, 1080, 1.0);
        assert_eq!(x, 485);
    }

    #[test]
    fn bob_amplitude_max_a_mi_periode() {
        let (_, y_haut) = position(1250, 1920, 1080, 1.0); // t = 2500/2
        assert_eq!(y_haut, (0.12 * 1080.0) as i32 - 8);
    }

    #[test]
    fn x_monotone_croissant() {
        let xs: Vec<i32> = (0..=12).map(|s| position(s * 1000, 1920, 1080, 1.0).0).collect();
        assert!(xs.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn fini_a_12s() {
        assert!(!finished(11_999));
        assert!(finished(12_000));
    }
}
