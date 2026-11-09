//! カードの編集面：描画、選択、移動、大きさ変更、吸着ガイド、配置ツール。

use egui::Color32;

mod draw;
mod geometry;
mod show;
mod snap;
mod state;
#[cfg(test)]
mod tests;
mod tool;
mod view;

pub use show::show;
pub use state::{CanvasAction, CanvasState};
pub use tool::Tool;
pub use view::View;

/// 1 インチあたりの画面ピクセル（倍率 100%）
pub const PX_PER_INCH: f32 = 96.0;
const RULER: f32 = 22.0;
const HANDLE: f32 = 7.0;
const SNAP_PX: f32 = 7.0;

pub const ACCENT: Color32 = Color32::from_rgb(0x1f, 0x6f, 0xeb);
const GUIDE: Color32 = Color32::from_rgb(0xe0, 0x3a, 0x8c);
const PHOTO_FILL: Color32 = Color32::from_rgb(0xe9, 0xef, 0xf6);
const PHOTO_EDGE: Color32 = Color32::from_rgb(0x8a, 0x9b, 0xb0);
