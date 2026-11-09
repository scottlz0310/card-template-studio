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
    pub(super) fn pos(self, r: Rect) -> Pos2 {
        let (x, y) = match self {
            Handle::NW => (r.left(), r.top()),
            Handle::N => (r.center().x, r.top()),
            Handle::NE => (r.right(), r.top()),
            Handle::E => (r.right(), r.center().y),
            Handle::SE => (r.right(), r.bottom()),
            Handle::S => (r.center().x, r.bottom()),
            Handle::SW => (r.left(), r.bottom()),
            Handle::W => (r.left(), r.center().y),
        };
        pos2(x, y)
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
