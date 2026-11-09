//! カード上の 1 つのオブジェクト。

use super::body::Body;
use super::{
    KIND_CAPTION, KIND_CM_VALUE, KIND_DRAFT, KIND_DRAW_OBJECT, KIND_PATTERN, KIND_PICTURE,
    KIND_POLY, KIND_RECT,
};

/// カード上の 1 つのオブジェクト（44 バイトのヘッダ + 本体）
#[derive(Clone, Debug, PartialEq)]
pub struct Object {
    /// タグ 'OBJn' の n（1..3）。読み込み時は +0x5c に入る（0x41af10）
    pub tag_version: u8,
    pub kind: u32,
    /// ヘッダ 2 番目の値（+0x88）。文字項目がどの写真に属するか（1 始まり, 0 = なし）
    pub link: u32,
    /// left, top, right, bottom（インチ。線は向きを保つため正規化しない）
    pub rect: [f64; 4],
    pub body: Body,
    /// OBJ3 のときだけ本体の後ろに続く u32（+0x8c, 0x40b200 / 0x40d230）
    pub trailer: Option<u32>,
}

impl Object {
    /// 新しい写真枠・文字項目を作る（タグの版はサンプルと本体の書き方に合わせる:
    /// 写真枠は版 10 = 'OBJ1'（0x42ae80）、文字項目は 'OBJ2'（0x407660））
    pub fn new(body: Body, rect: [f64; 4]) -> Self {
        let (kind, tag_version) = match &body {
            Body::Picture(_) => (KIND_PICTURE, 1),
            Body::Caption(_) => (KIND_CAPTION, 2),
            Body::CmValue(_) => (KIND_CM_VALUE, 1),
            Body::Passthrough(_) => unreachable!("透過保持のオブジェクトは読み込みでしか作らない"),
        };
        Self {
            tag_version,
            kind,
            link: 0,
            rect,
            body,
            trailer: None,
        }
    }

    /// 本体が付ける「版」（10/11/12）。ヘッダ sub_version の計算に使う
    pub fn object_version(&self) -> u32 {
        match (&self.body, self.kind) {
            // 図形の版は常に 12（0x421050）
            (Body::Passthrough(_), KIND_RECT | KIND_PATTERN | KIND_POLY) => 12,
            // 文字項目は +0x8c==1 のとき 12、それ以外 10（0x41af30）。'OBJ2' は 10 の印
            (Body::Caption(_), _) => {
                if self.tag_version >= 3 {
                    12
                } else {
                    10
                }
            }
            _ => match self.tag_version {
                0 | 1 => 10,
                2 => 11,
                _ => 12,
            },
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match (&self.body, self.kind) {
            (Body::Picture(_), _) => "写真枠",
            (Body::Caption(c), _) => {
                if c.is_value != 0 {
                    "項目値"
                } else {
                    "文字"
                }
            }
            (Body::CmValue(_), _) => "施工管理値",
            (Body::Passthrough(_), KIND_DRAW_OBJECT) => "部品",
            (Body::Passthrough(_), KIND_DRAFT) => "参考図",
            (Body::Passthrough(_), _) => {
                self.shape_view().map(|s| s.shape.label()).unwrap_or("図形")
            }
        }
    }

    /// 透過保持していて編集できないオブジェクトか
    pub fn is_passthrough(&self) -> bool {
        matches!(self.body, Body::Passthrough(_))
    }

    pub fn normalized(&self) -> [f64; 4] {
        let [l, t, r, b] = self.rect;
        [l.min(r), t.min(b), l.max(r), t.max(b)]
    }
}
