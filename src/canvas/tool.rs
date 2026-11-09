//! 配置ツールと、ドラッグで部品を置く操作。

use crate::doc::Document;
use crate::tpl::MM_PER_INCH;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Picture,
    Label,
    Value,
    Row,
}

impl Tool {
    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "選択",
            Tool::Picture => "写真枠",
            Tool::Label => "項目名・文字",
            Tool::Value => "項目値",
            Tool::Row => "項目の行（名＋値）",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            Tool::Select => {
                "クリックで選択、ドラッグで移動。Shift で追加選択、何もない所をドラッグで範囲選択"
            }
            Tool::Picture => "カード上をドラッグして写真枠を描きます",
            Tool::Label => "ドラッグして固定の文字（項目名など）を置きます",
            Tool::Value => "ドラッグしてデータを表示する欄（項目値）を置きます",
            Tool::Row => "ドラッグして「項目名＋項目値」の 1 行を置きます",
        }
    }
}

pub(super) fn default_size(tool: Tool) -> (f64, f64) {
    let mm = |v: f64| v / MM_PER_INCH;
    match tool {
        Tool::Picture => (mm(80.0), mm(60.0)),
        Tool::Row => (mm(70.0), mm(5.0)),
        _ => (mm(25.0), mm(5.0)),
    }
}

pub(super) fn place(doc: &mut Document, tool: Tool, a: (f64, f64), b: (f64, f64)) {
    let mut r = [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)];
    let (dw, dh) = default_size(tool);
    // 小さすぎる（クリックに近い）ときは既定の大きさ
    if r[2] - r[0] < 2.0 / MM_PER_INCH {
        r[2] = r[0] + dw;
    }
    if r[3] - r[1] < 2.0 / MM_PER_INCH {
        r[3] = r[1] + dh;
    }
    match tool {
        Tool::Picture => {
            doc.add_picture(r);
        }
        Tool::Label => {
            doc.add_caption("項目名", "", false, r);
        }
        Tool::Value => {
            doc.add_caption("", "写真タイトル", true, r);
        }
        Tool::Row => {
            let w = r[2] - r[0];
            let (a, b) = doc.add_field_row(
                "写真タイトル",
                (r[0], r[1]),
                w * 0.32,
                w * 0.68,
                r[3] - r[1],
            );
            doc.selection = [a, b].into();
        }
        Tool::Select => {}
    }
}
