//! 表示の状態（拡大・位置）と、インチ座標と画面座標の変換。

use egui::{Pos2, Rect, Vec2, vec2};

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
