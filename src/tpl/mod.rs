//! 現場編集長アルバム（GhAlbum.exe）のカードテンプレート（.TPL）形式の読み書き。
//!
//! 形式の根拠は MECHANICS.md（関数アドレス付き）を参照。未解明の値も含めて
//! 読み込んだバイトをそのまま保持し、編集しない限り同じバイト列で書き戻す。
//!
//! 責務ごとのサブモジュールに分けてあり、外部に見せる名前はここから再公開する。
//! - `text`: cp932 の文字列
//! - `io`: バイト列の読み手・書き手
//! - `header` / `export`: TMP1 ヘッダ、本体のエクスポート形式（expinf, フォルダ名）
//! - `format`: 文字の書式（44 バイト）と色
//! - `body` / `object` / `shape`: オブジェクトとその本体、透過保持の図形
//! - `codec` / `template`: 読み書きとテンプレート全体

mod body;
mod codec;
mod error;
mod export;
mod format;
mod header;
mod io;
mod object;
mod shape;
#[cfg(test)]
mod survey;
mod template;
#[cfg(test)]
mod tests;
mod text;

pub use body::*;
pub use export::*;
pub use format::*;
pub use header::*;
pub use object::*;
pub use shape::*;
pub use template::*;
pub use text::*;

/// 1 インチ = 25.4 mm（座標はインチで保存されている）
pub const MM_PER_INCH: f64 = 25.4;

pub const KIND_PICTURE: u32 = 2;
pub const KIND_CAPTION: u32 = 3;
pub const KIND_DRAW_OBJECT: u32 = 4;
pub const KIND_CM_VALUE: u32 = 5;
pub const KIND_DRAFT: u32 = 6;
pub const KIND_RECT: u32 = 7;
pub const KIND_PATTERN: u32 = 8;
pub const KIND_POLY: u32 = 9;
