//! 操作中の状態と、呼び出し側へ返す操作。

use super::geometry::Handle;
use super::tool::Tool;
use egui::Pos2;

pub(super) enum Drag {
    Move {
        start: (f64, f64),
        origin: Vec<(usize, [f64; 4])>,
        moved: bool,
    },
    Resize {
        idx: usize,
        handle: Handle,
        orig: [f64; 4],
    },
    Marquee {
        start: Pos2,
        additive: bool,
    },
    Pan {
        last: Pos2,
    },
    Place {
        tool: Tool,
        start: (f64, f64),
    },
}

#[derive(Default)]
pub struct CanvasState {
    pub(super) drag: Option<Drag>,
    pub(super) guides_x: Vec<f64>,
    pub(super) guides_y: Vec<f64>,
    pub cursor_inch: Option<(f64, f64)>,
    /// ダブルクリックで文字欄を編集したい合図（プロパティ欄が拾う）
    pub focus_text: bool,
    pub status: Option<String>,
}

pub enum CanvasAction {
    None,
    ContextDuplicate,
    ContextDelete,
    ContextFront,
    ContextBack,
}
