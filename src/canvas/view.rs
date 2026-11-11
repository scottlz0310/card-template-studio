//! 表示の状態（拡大・位置）と、インチ座標と画面座標の変換。

use super::PX_PER_INCH;
use egui::{Pos2, Rect, Vec2, vec2};

/// 方眼の間隔（mm）として選べる範囲。メニューの入力欄と、保存した設定の検査で共通に使う
pub const GRID_MM_RANGE: std::ops::RangeInclusive<f32> = 0.5..=20.0;

pub struct View {
    pub zoom: f32,
    pub pan: Vec2,
    pub fit_pending: bool,
    pub grid_mm: f32,
    pub show_grid: bool,
    pub snap_grid: bool,
    pub snap_objects: bool,
    pub show_used: bool,
    /// 項目値の欄に見本の文字を表示する
    pub sample_values: bool,
}

impl Default for View {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            fit_pending: true,
            grid_mm: 1.0,
            show_grid: true,
            snap_grid: true,
            snap_objects: true,
            show_used: true,
            sample_values: false,
        }
    }
}

pub(super) struct Xf {
    pub(super) origin: Pos2,
    pub(super) ppi: f32,
}

impl Xf {
    pub(super) fn pt(&self, x: f64, y: f64) -> Pos2 {
        self.origin + vec2(x as f32 * self.ppi, y as f32 * self.ppi)
    }
    pub(super) fn rect(&self, r: [f64; 4]) -> Rect {
        Rect::from_two_pos(self.pt(r[0], r[1]), self.pt(r[2], r[3]))
    }
    pub(super) fn inch(&self, p: Pos2) -> (f64, f64) {
        (
            ((p.x - self.origin.x) / self.ppi) as f64,
            ((p.y - self.origin.y) / self.ppi) as f64,
        )
    }
}

/// 拡大率の範囲
pub(super) const ZOOM_MIN: f32 = 0.2;
pub(super) const ZOOM_MAX: f32 = 8.0;

/// カード全体が作業領域に収まる拡大率と、カードを中央に置く位置。
/// `work` は作業領域の大きさ（px）、`card` はカードの大きさ（インチ）。
pub(super) fn fit(work: (f32, f32), card: (f64, f64)) -> (f32, (f32, f32)) {
    let zx = (work.0 - 80.0) / (card.0 as f32 * PX_PER_INCH);
    let zy = (work.1 - 80.0) / (card.1 as f32 * PX_PER_INCH);
    let zoom = zx.min(zy).clamp(ZOOM_MIN, ZOOM_MAX);
    let ppi = PX_PER_INCH * zoom;
    (
        zoom,
        (
            (work.0 - card.0 as f32 * ppi) / 2.0,
            (work.1 - card.1 as f32 * ppi) / 2.0,
        ),
    )
}

/// ポインタの下の点を動かさずに拡大率を `delta` 倍する。
/// `pointer` は作業領域の左上から見たポインタの位置（px）。
pub(super) fn zoom_at(
    zoom: f32,
    pan: (f32, f32),
    delta: f32,
    pointer: (f32, f32),
) -> (f32, (f32, f32)) {
    let new = (zoom * delta).clamp(ZOOM_MIN, ZOOM_MAX);
    let anchor = (pointer.0 - pan.0, pointer.1 - pan.1);
    let k = new / zoom;
    (
        new,
        (
            pan.0 + (anchor.0 - anchor.0 * k),
            pan.1 + (anchor.1 - anchor.1 * k),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_picks_the_tighter_axis_and_centers_the_card() {
        // (作業領域 px, カード インチ, 期待する拡大率)
        let cases = [
            // 横が窮屈: (1000-80)/(10*96)
            ((1000.0, 2000.0), (10.0, 4.0), 920.0 / 960.0),
            // 縦が窮屈: (500-80)/(4*96)
            ((2000.0, 500.0), (10.0, 4.0), 420.0 / 384.0),
        ];
        for ((w, h), (cw, ch), zoom) in cases {
            let (z, (px, py)) = fit((w, h), (cw, ch));
            assert!((z - zoom).abs() < 1e-5, "{z} != {zoom}");
            let ppi = PX_PER_INCH * z;
            assert!((px - (w - cw as f32 * ppi) / 2.0).abs() < 1e-3);
            assert!((py - (h - ch as f32 * ppi) / 2.0).abs() < 1e-3);
        }
    }

    #[test]
    fn fit_clamps_the_zoom() {
        assert_eq!(fit((100000.0, 100000.0), (1.0, 1.0)).0, ZOOM_MAX);
        assert_eq!(fit((90.0, 90.0), (100.0, 100.0)).0, ZOOM_MIN);
    }

    #[test]
    fn zoom_at_keeps_the_point_under_the_pointer_fixed() {
        let (zoom, pan, pointer) = (1.0_f32, (30.0_f32, 20.0_f32), (200.0_f32, 150.0_f32));
        for delta in [0.5_f32, 1.25, 2.0] {
            let (z, p) = zoom_at(zoom, pan, delta, pointer);
            assert!((z - zoom * delta).abs() < 1e-6);
            // カード上の点 = (pointer - pan) / zoom は、拡大の前後で同じ
            let before = ((pointer.0 - pan.0) / zoom, (pointer.1 - pan.1) / zoom);
            let after = ((pointer.0 - p.0) / z, (pointer.1 - p.1) / z);
            assert!((before.0 - after.0).abs() < 1e-3 && (before.1 - after.1).abs() < 1e-3);
        }
    }

    #[test]
    fn zoom_at_clamps_and_stays_put_at_the_limit() {
        let (z, p) = zoom_at(ZOOM_MAX, (10.0, 10.0), 4.0, (100.0, 100.0));
        assert_eq!(z, ZOOM_MAX);
        assert_eq!(p, (10.0, 10.0), "上限では動かない");
        let (z, _) = zoom_at(0.25, (0.0, 0.0), 0.1, (0.0, 0.0));
        assert_eq!(z, ZOOM_MIN);
    }
}
