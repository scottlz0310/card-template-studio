//! 矩形の計算と、大きさ変更のハンドル。

use egui::{Pos2, Rect, pos2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Handle {
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
}

impl Handle {
    pub(super) const ALL: [Handle; 8] = [
        Handle::NW,
        Handle::N,
        Handle::NE,
        Handle::E,
        Handle::SE,
        Handle::S,
        Handle::SW,
        Handle::W,
    ];
    /// ハンドルの位置。矩形は `[left, top, right, bottom]`（単位は問わない）
    pub(super) fn at(self, r: [f64; 4]) -> (f64, f64) {
        let [l, t, ri, b] = r;
        let (cx, cy) = ((l + ri) / 2.0, (t + b) / 2.0);
        match self {
            Handle::NW => (l, t),
            Handle::N => (cx, t),
            Handle::NE => (ri, t),
            Handle::E => (ri, cy),
            Handle::SE => (ri, b),
            Handle::S => (cx, b),
            Handle::SW => (l, b),
            Handle::W => (l, cy),
        }
    }
    pub(super) fn pos(self, r: Rect) -> Pos2 {
        let (x, y) = self.at([
            r.left() as f64,
            r.top() as f64,
            r.right() as f64,
            r.bottom() as f64,
        ]);
        pos2(x as f32, y as f32)
    }
    pub(super) fn moves(self) -> (bool, bool, bool, bool) {
        // left, top, right, bottom
        match self {
            Handle::N => (false, true, false, false),
            Handle::S => (false, false, false, true),
            Handle::E => (false, false, true, false),
            Handle::W => (true, false, false, false),
            Handle::NE => (false, true, true, false),
            Handle::NW => (true, true, false, false),
            Handle::SE => (false, false, true, true),
            Handle::SW => (true, false, false, true),
        }
    }
    pub(super) fn cursor(self) -> egui::CursorIcon {
        match self {
            Handle::N | Handle::S => egui::CursorIcon::ResizeVertical,
            Handle::E | Handle::W => egui::CursorIcon::ResizeHorizontal,
            Handle::NE | Handle::SW => egui::CursorIcon::ResizeNeSw,
            Handle::NW | Handle::SE => egui::CursorIcon::ResizeNwSe,
        }
    }
}

pub(super) fn norm(r: [f64; 4]) -> [f64; 4] {
    [
        r[0].min(r[2]),
        r[1].min(r[3]),
        r[0].max(r[2]),
        r[1].max(r[3]),
    ]
}

pub(super) fn bounds_of(it: impl Iterator<Item = [f64; 4]>) -> [f64; 4] {
    it.fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |a, b| {
        [
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[2].max(b[2]),
            a[3].max(b[3]),
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_at_matches_the_rect_based_position() {
        let r = [10.0, 20.0, 30.0, 60.0];
        let rect = Rect::from_min_max(pos2(10.0, 20.0), pos2(30.0, 60.0));
        for h in Handle::ALL {
            let (x, y) = h.at(r);
            assert_eq!(h.pos(rect), pos2(x as f32, y as f32), "{h:?}");
        }
        assert_eq!(Handle::N.at(r), (20.0, 20.0));
        assert_eq!(Handle::SE.at(r), (30.0, 60.0));
        assert_eq!(Handle::W.at(r), (10.0, 40.0));
    }
}
