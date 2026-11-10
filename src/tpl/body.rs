//! 写真枠・文字項目・施工管理値の本体。

use super::format::{FMT_LEN, default_format};
use super::text::{SjisString, string_to_fixed};

/// 写真枠（種類 2, CPictureObject 0x42f840 / 0x42d610）
#[derive(Clone, Debug, PartialEq)]
pub struct PictureBody {
    pub crop: [u8; 32],
    /// +0xb8: 下位 4 ビット = 枠の形（`PictureShape`）、bit 4〜7 = 枠線の太さ（`PictureBorder`）。
    /// それより上のビットは見た目に効かないが、読んだまま保持する（実機の差分実験、#18 / #25）
    pub frame_style: u32,
    /// +0xbc: 枠線の色（COLORREF）。枠線の太さが 0 のときは使われない
    pub border_color: u32,
    /// 写真の GUID（テンプレートでは空か "0"）
    pub guid: SjisString,
    pub c4: SjisString,
    /// 写真ファイルのパス
    pub path: SjisString,
    pub cc: u32,
    pub d0: u32,
    pub d4: u32,
    pub d8: u32,
    pub dc: SjisString,
    pub e0: u32,
    pub e4: SjisString,
    pub e8: u32,
    /// 版 11 以上（OBJ2/OBJ3）のときだけ保存される
    pub ec: Option<u32>,
}

impl Default for PictureBody {
    fn default() -> Self {
        Self {
            crop: [0; 32],
            frame_style: 0,
            border_color: 0,
            guid: SjisString::default(),
            c4: SjisString::default(),
            path: SjisString::default(),
            cc: 0,
            d0: 0,
            d4: 0,
            d8: 0,
            dc: SjisString::default(),
            e0: 0xffff_ffff,
            e4: SjisString::default(),
            e8: 3,
            ec: None,
        }
    }
}

/// 新規の文字項目のフォント。本体（Ver.25）が新規に作る文字項目と同じ
pub const DEFAULT_FACE: &str = "メイリオ";

/// 写真枠の形（`frame_style` の下位 4 ビット）。範囲外（4〜15）は本体が四角で描く
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PictureShape {
    Rect,
    Rounded,
    Ellipse,
}

impl PictureShape {
    pub const ALL: [Self; 3] = [Self::Rect, Self::Rounded, Self::Ellipse];

    pub fn label(self) -> &'static str {
        match self {
            Self::Rect => "四角",
            Self::Rounded => "角丸",
            Self::Ellipse => "楕円",
        }
    }

    /// 書き込む値（`3` も楕円に見えるが、書き戻すのは `2`）
    fn bits(self) -> u32 {
        match self {
            Self::Rect => 0,
            Self::Rounded => 1,
            Self::Ellipse => 2,
        }
    }
}

/// 写真枠の枠線の太さ（`frame_style` の bit 4〜7）。範囲外（4〜15）は枠線なし
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PictureBorder {
    None,
    Thin,
    Medium,
    Thick,
}

impl PictureBorder {
    pub const ALL: [Self; 4] = [Self::None, Self::Thin, Self::Medium, Self::Thick];

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "なし",
            Self::Thin => "細い",
            Self::Medium => "中",
            Self::Thick => "太い",
        }
    }

    fn bits(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Thin => 1,
            Self::Medium => 2,
            Self::Thick => 3,
        }
    }
}

impl PictureBody {
    pub fn shape(&self) -> PictureShape {
        match self.frame_style & 0xf {
            1 => PictureShape::Rounded,
            2 | 3 => PictureShape::Ellipse,
            _ => PictureShape::Rect,
        }
    }

    /// 形だけを変える（枠線の太さとそれより上のビットは保つ）
    pub fn set_shape(&mut self, shape: PictureShape) {
        self.frame_style = (self.frame_style & !0xf) | shape.bits();
    }

    pub fn border(&self) -> PictureBorder {
        match (self.frame_style >> 4) & 0xf {
            1 => PictureBorder::Thin,
            2 => PictureBorder::Medium,
            3 => PictureBorder::Thick,
            _ => PictureBorder::None,
        }
    }

    /// 枠線の太さだけを変える（形とそれより上のビットは保つ）
    pub fn set_border(&mut self, border: PictureBorder) {
        self.frame_style = (self.frame_style & !0xf0) | (border.bits() << 4);
    }
}

/// 文字項目（種類 3, CCaptionObject 0x407400 / 0x408610）
#[derive(Clone, Debug, PartialEq)]
pub struct CaptionBody {
    /// 表示する文字（項目名ラベルの文字）
    pub text: SjisString,
    pub face: [u8; 32],
    pub fmt: [u8; FMT_LEN],
    /// アルバム上の値（テンプレートでは "0"）
    pub value: SjisString,
    /// データベースの項目名（"写真タイトル" など）
    pub field: SjisString,
    /// +0x108（意味未確定。0/1。`is_value` が -1 の 14 件は 14 件とも -1）
    pub t108: i32,
    /// +0x10c: 1 = 項目値（データを表示）, 0 = 項目名, -1 = 項目に結び付かない固定文字
    /// （実物 563 件で、-1 は 14 件すべて `field` が空・表示文字あり。実機でも固定文字として表示。#18 / #22）
    pub is_value: i32,
    /// +0x110: 枠線あり（0x408b30, 読み込み時 0..1 に丸める）
    pub border: i32,
    /// +0x114: 0..2（1/2 で枠を 0.05/0.1 内側へ, 0x408b30）
    pub border_inset: i32,
    /// +0x118 / +0x11c: 版 2 以上でのみ保存（枠線色, 太さ 0/1/2 = 0.1/0.3/0.7mm）
    pub border_style: Option<(u32, i32)>,
}

impl CaptionBody {
    pub fn new(text: &str, field: &str, is_value: bool, size_pt: u16) -> Self {
        Self {
            text: SjisString::from_str(text),
            face: string_to_fixed(DEFAULT_FACE),
            fmt: default_format(size_pt),
            value: SjisString::from_str("0"),
            field: SjisString::from_str(field),
            t108: 0,
            is_value: is_value as i32,
            border: 1,
            border_inset: 0,
            border_style: Some((0, 0)),
        }
    }

    /// 項目値（データを表示する枠）か。`0`（項目名）と `-1`（固定文字）は表示文字を描く側
    pub fn is_value_cell(&self) -> bool {
        self.is_value == 1
    }
}

/// 施工管理値（種類 5, CCMValueObject 0x40f8a0）
#[derive(Clone, Debug, PartialEq)]
pub struct CmValueBody {
    pub text: SjisString,
    pub c0: u32,
    pub c4: u32,
    pub x138: u32,
    pub x13c: u32,
    pub x148: u32,
    pub face: [u8; 32],
    pub fmt: [u8; FMT_LEN],
    pub x140: u32,
    pub x154: u32,
    pub x150: u32,
    pub x14c: u32,
    pub x144: u32,
    /// 版 11 以上: +0x15c, +0x160, 行（各 68 バイト: u32 + 32 + 32）
    pub ext: Option<(u32, u32, Vec<[u8; 68]>)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    Picture(PictureBody),
    Caption(CaptionBody),
    CmValue(CmValueBody),
    /// 種類 4・6・7・8・9（部品・参考図・図形）。サンプルに無く編集対象外なので、
    /// 読み込んだ本体のバイト列をそのまま保持して書き戻す。
    Passthrough(Vec<u8>),
}
