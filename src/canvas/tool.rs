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
    /// 表組（複数行）。項目と列の比率は `TableSpec`
    Table,
}

/// 「表組の挿入」で決めた内容。項目を上から並べ、項目名の列は幅の `label_pct` %
#[derive(Clone, Debug, PartialEq)]
pub struct TableSpec {
    pub fields: Vec<String>,
    pub label_pct: u32,
}

impl Default for TableSpec {
    fn default() -> Self {
        Self {
            fields: crate::doc::TABLE_PRESETS[0]
                .1
                .iter()
                .map(|f| (*f).to_string())
                .collect(),
            label_pct: 32,
        }
    }
}

/// 表組の 1 行の既定の高さ（mm）。クリックで置くときの高さは、これに行数を掛ける
const TABLE_ROW_MM: f64 = 5.0;

impl Tool {
    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "選択",
            Tool::Picture => "写真枠",
            Tool::Label => "項目名・文字",
            Tool::Value => "項目値",
            Tool::Row => "項目の行（名＋値）",
            Tool::Table => "表組",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            Tool::Select => {
                "クリックで選択、ドラッグで移動。Ctrl+クリックで追加・解除、Shift+クリックで範囲選択、何もない所をドラッグで範囲選択"
            }
            Tool::Picture => "カード上をドラッグして写真枠を描きます",
            Tool::Label => "ドラッグして固定の文字（項目名など）を置きます",
            Tool::Value => "ドラッグしてデータを表示する欄（項目値）を置きます",
            Tool::Row => "ドラッグして「項目名＋項目値」の 1 行を置きます",
            Tool::Table => {
                "ドラッグした範囲に、決めた項目の表組を等分して置きます（クリックなら既定の大きさ）"
            }
        }
    }
}

pub(super) fn default_size(tool: Tool, table: &TableSpec) -> (f64, f64) {
    let mm = |v: f64| v / MM_PER_INCH;
    match tool {
        Tool::Picture => (mm(80.0), mm(60.0)),
        Tool::Row => (mm(70.0), mm(5.0)),
        Tool::Table => (
            mm(70.0),
            mm(TABLE_ROW_MM * table.fields.len().max(1) as f64),
        ),
        _ => (mm(25.0), mm(5.0)),
    }
}

pub(super) fn place(
    doc: &mut Document,
    tool: Tool,
    a: (f64, f64),
    b: (f64, f64),
    table: &TableSpec,
) {
    let mut r = [a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)];
    let (dw, dh) = default_size(tool, table);
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
        Tool::Table => {
            doc.add_field_table(&table.fields, r, table.label_pct as f64 / 100.0);
        }
        Tool::Select => {}
    }
}
