//! 透過保持する図形・部品。中身は解釈せず、画面に描くためと長さを知るためにだけ読む。

use super::body::Body;
use super::error::Result;
use super::io::Reader;
use super::object::Object;
use super::{KIND_DRAFT, KIND_DRAW_OBJECT, KIND_PATTERN, KIND_POLY, KIND_RECT};

/// 図形の共通部分（CDrawObj 0x421230 / 0x421170。MFC DRAWCLI と同じ並び）。表示のためだけに読む
#[derive(Clone, Debug, PartialEq)]
pub struct DrawBase {
    pub raw_90: u32,
    pub raw_94: u32,
    pub raw_98: u8,
    /// +0x9c: 線あり
    pub pen: u32,
    /// LOGPEN: 線種, 太さ x (pt, 0x421380 で /72), 太さ y, 色
    pub pen_style: u32,
    pub pen_width: i32,
    pub pen_width_y: i32,
    pub pen_color: u32,
    /// +0xb0: 塗りつぶしあり
    pub brush: u32,
    /// LOGBRUSH: 種類, 色, ハッチ
    pub brush_style: u32,
    pub brush_color: u32,
    pub brush_hatch: u32,
}

/// 図形の形（CDrawRect +0xc0, 0x4247a0 の分岐）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Rectangle,
    RoundRect,
    Ellipse,
    Line,
    Arrow,
    Polygon,
}

impl ShapeKind {
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::RoundRect,
            2 => Self::Ellipse,
            3 => Self::Line,
            4 => Self::Arrow,
            _ => Self::Rectangle,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Rectangle => "四角形",
            Self::RoundRect => "角丸四角形",
            Self::Ellipse => "楕円",
            Self::Line => "直線",
            Self::Arrow => "矢印",
            Self::Polygon => "多角形",
        }
    }
}

/// 透過保持している図形を画面に描くための読み取り結果
#[derive(Clone, Debug)]
pub struct ShapeView {
    pub shape: ShapeKind,
    pub base: DrawBase,
    /// 角丸の比率（幅・高さに掛ける）
    pub round: (f64, f64),
    /// 多角形の点（インチ）
    pub points: Vec<(f64, f64)>,
}

impl Object {
    /// 透過保持の図形（種類 7・8・9）を画面に描くために読む。読めなければ None
    pub fn shape_view(&self) -> Option<ShapeView> {
        let Body::Passthrough(raw) = &self.body else {
            return None;
        };
        let mut r = Reader::new(raw);
        let base = read_base(&mut r).ok()?;
        match self.kind {
            KIND_RECT | KIND_PATTERN => {
                let shape = ShapeKind::from_u32(r.u32().ok()?);
                for _ in 0..4 {
                    r.u32().ok()?;
                }
                let round = (r.f64().ok()?, r.f64().ok()?);
                Some(ShapeView {
                    shape,
                    base,
                    round,
                    points: Vec::new(),
                })
            }
            KIND_POLY => {
                let n = r.u32().ok()? as usize;
                let mut points = Vec::with_capacity(n.min(10_000));
                for _ in 0..n.min(10_000) {
                    points.push((r.f64().ok()?, r.f64().ok()?));
                }
                Some(ShapeView {
                    shape: ShapeKind::Polygon,
                    base,
                    round: (0.0, 0.0),
                    points,
                })
            }
            _ => None,
        }
    }
}

pub(super) fn read_base(r: &mut Reader) -> Result<DrawBase> {
    Ok(DrawBase {
        raw_90: r.u32()?,
        raw_94: r.u32()?,
        raw_98: r.u8()?,
        pen: r.u32()?,
        pen_style: r.u32()?,
        pen_width: r.i32()?,
        pen_width_y: r.i32()?,
        pen_color: r.u32()?,
        brush: r.u32()?,
        brush_style: r.u32()?,
        brush_color: r.u32()?,
        brush_hatch: r.u32()?,
    })
}

/// 透過保持する本体を読み飛ばす（中身は保持するだけで解釈しない）
pub(super) fn skip_passthrough_body(r: &mut Reader, kind: u32) -> Result<()> {
    match kind {
        // CDrawObject: u16 ×3, 8 バイト, u32 ×2（0x426840）
        KIND_DRAW_OBJECT => {
            r.bytes(22)?;
        }
        // CDraftObject: 32 バイト, u32 ×2, 文字列 ×2, u32 ×4, 文字列, u32, 文字列, u32（0x41d320）
        KIND_DRAFT => {
            r.bytes(32 + 8)?;
            r.cstring()?;
            r.cstring()?;
            r.bytes(16)?;
            r.cstring()?;
            r.bytes(4)?;
            r.cstring()?;
            r.bytes(4)?;
        }
        // CDrawRect: 共通 45 バイト + u32 ×5 + double ×2（0x421640）。CDrawPattern は + u32（0x421870）
        KIND_RECT | KIND_PATTERN => {
            read_base(r)?;
            r.bytes(20 + 16)?;
            if kind == KIND_PATTERN {
                r.bytes(4)?;
            }
        }
        // CDrawPoly: 共通 45 バイト + 点の数 + 点 ×（double ×2）（0x421b00）
        _ => {
            read_base(r)?;
            let n = r.u32()? as usize;
            if n > 100_000 {
                return Err(r.err("点の数が大きすぎます"));
            }
            r.bytes(n * 16)?;
        }
    }
    Ok(())
}
