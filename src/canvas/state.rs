//! 操作中の状態と、呼び出し側へ返す操作。

use super::geometry::Handle;
use super::tool::{TableSpec, Tool};

/// ドラッグ中の操作。座標はインチ（`Pan` だけは画面の px）
pub(super) enum Drag {
    Move {
        start: (f64, f64),
        origin: Vec<(usize, [f64; 4])>,
    },
    Resize {
        idx: usize,
        handle: Handle,
        orig: [f64; 4],
    },
    Marquee {
        start: (f64, f64),
    },
    Pan {
        last: (f32, f32),
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
    /// 表組ツールで置く内容（「表組の挿入」で決める）
    pub table: TableSpec,
}

pub enum CanvasAction {
    None,
    ContextDuplicate,
    ContextDelete,
    ContextFront,
    ContextBack,
}
