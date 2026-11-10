//! 開いているテンプレート（文書）と、その編集操作・ファイル操作。画面に依存しない。

use crate::tpl::{
    self, Body, CaptionBody, EXPINF_TEXT, Header, KIND_CAPTION, KIND_PICTURE, Object, PictureBody,
    SjisString, Template,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// 本体のサンプルで使われている項目名（データベースの項目）
pub const KNOWN_FIELDS: &[&str] = &[
    "写真タイトル",
    "撮影箇所",
    "撮影年月日",
    "工種",
    "種別",
    "細別",
    "請負者説明文",
    "施工管理値",
    "(自由入力1)",
    "撮影情報予備",
    "撮影情報予備1",
    "工種区分予備1",
    "工種区分予備2",
    "写真ファイル日本語名",
    "写真-大分類",
    "写真区分",
];

/// テンプレート名に使えない文字（本体の「テンプレート名に使用できない文字が含まれています」）
pub const INVALID_NAME_CHARS: &[char] = &['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

pub fn validate_name(name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("テンプレート名を入力してください。".into());
    }
    if name
        .chars()
        .any(|c| INVALID_NAME_CHARS.contains(&c) || c.is_control())
    {
        return Err(
            "テンプレート名に使用できない文字（\\ / : * ? \" < > |）が含まれています。".into(),
        );
    }
    if tpl::sjis_unmappable(name) {
        return Err("テンプレート名に本体で扱えない文字が含まれています。".into());
    }
    if name.chars().count() > 100 {
        return Err("テンプレート名が長すぎます。".into());
    }
    Ok(())
}

#[derive(Clone)]
struct Snapshot {
    template: Template,
    selection: BTreeSet<usize>,
}

pub struct Document {
    pub template: Template,
    pub name: String,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub selection: BTreeSet<usize>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// 同じ操作の連続（入力欄の打鍵など）を 1 回の「元に戻す」にまとめる印
    coalesce: Option<String>,
}

impl Document {
    pub fn new(template: Template, name: impl Into<String>, path: Option<PathBuf>) -> Self {
        Self {
            template,
            name: name.into(),
            path,
            dirty: false,
            selection: BTreeSet::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            coalesce: None,
        }
    }

    pub fn open(path: &Path) -> Result<Self, String> {
        let data =
            std::fs::read(path).map_err(|e| format!("{} を読めません: {e}", path.display()))?;
        let template = Template::parse(&data)
            .map_err(|e| format!("{} はテンプレートとして読めません（{e}）", path.display()))?;
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Self::new(template, name, Some(path.to_path_buf())))
    }

    /// 変更の前に呼ぶ。`key` が直前と同じなら新しい段を作らない（None は常に新しい段）
    pub fn checkpoint(&mut self, key: Option<&str>) {
        if let (Some(k), Some(prev)) = (key, &self.coalesce)
            && k == prev
        {
            self.dirty = true;
            return;
        }
        self.coalesce = key.map(str::to_owned);
        self.undo.push(Snapshot {
            template: self.template.clone(),
            selection: self.selection.clone(),
        });
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    pub fn end_coalesce(&mut self) {
        self.coalesce = None;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some(s) = self.undo.pop() {
            self.redo.push(Snapshot {
                template: self.template.clone(),
                selection: self.selection.clone(),
            });
            self.template = s.template;
            self.selection = s.selection;
            self.dirty = true;
            self.coalesce = None;
        }
    }

    pub fn redo(&mut self) {
        if let Some(s) = self.redo.pop() {
            self.undo.push(Snapshot {
                template: self.template.clone(),
                selection: self.selection.clone(),
            });
            self.template = s.template;
            self.selection = s.selection;
            self.dirty = true;
            self.coalesce = None;
        }
    }

    pub fn save_to(&mut self, path: &Path) -> Result<(), String> {
        let bytes = self.template.to_bytes();
        // 書き戻した内容が読めることを確かめてから保存する
        Template::parse(&bytes).map_err(|e| format!("保存する内容の確認に失敗しました（{e}）"))?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("{} を作れません: {e}", dir.display()))?;
        }
        std::fs::write(path, &bytes)
            .map_err(|e| format!("{} に保存できません: {e}", path.display()))?;
        self.path = Some(path.to_path_buf());
        if let Some(stem) = path.file_stem() {
            self.name = stem.to_string_lossy().into_owned();
        }
        self.dirty = false;
        Ok(())
    }

    // ----- 選択 -----

    pub fn editable(&self, i: usize) -> bool {
        self.template
            .objects
            .get(i)
            .is_some_and(|o| !o.is_passthrough())
    }

    pub fn selected_editable(&self) -> Vec<usize> {
        self.selection
            .iter()
            .copied()
            .filter(|&i| self.editable(i))
            .collect()
    }

    pub fn select_all(&mut self) {
        self.selection = (0..self.template.objects.len()).collect();
    }

    // ----- 追加・削除 -----

    /// 既存の同じ役割の物から本体（書式や未解明の値）を引き継いで新しい物を作る
    pub fn add_picture(&mut self, rect: [f64; 4]) -> usize {
        self.checkpoint(None);
        let body = self
            .template
            .objects
            .iter()
            .find_map(|o| match &o.body {
                Body::Picture(p) if o.tag_version == 1 => Some(p.clone()),
                _ => None,
            })
            .unwrap_or_default();
        let body = PictureBody {
            guid: SjisString::default(),
            path: SjisString::default(),
            ..body
        };
        self.push(Object::new(Body::Picture(body), rect))
    }

    pub fn add_caption(
        &mut self,
        text: &str,
        field: &str,
        is_value: bool,
        rect: [f64; 4],
    ) -> usize {
        self.checkpoint(None);
        let template_caption = self.template.objects.iter().find_map(|o| match &o.body {
            Body::Caption(c) if c.is_value == is_value as i32 && o.tag_version == 2 => {
                Some(c.clone())
            }
            _ => None,
        });
        let body = match template_caption {
            Some(c) => CaptionBody {
                text: SjisString::from_str(text),
                field: SjisString::from_str(field),
                value: SjisString::from_str("0"),
                ..c
            },
            None => CaptionBody::new(text, field, is_value, 12),
        };
        let link = self.default_link();
        let i = self.push(Object::new(Body::Caption(body), rect));
        self.template.objects[i].link = link;
        i
    }

    /// 項目名と項目値を 1 行ぶん並べて追加する（サンプルの「表組」と同じ形）
    pub fn add_field_row(
        &mut self,
        field: &str,
        origin: (f64, f64),
        label_w: f64,
        value_w: f64,
        h: f64,
    ) -> (usize, usize) {
        let (x, y) = origin;
        let a = self.add_caption(field, field, false, [x, y, x + label_w, y + h]);
        self.coalesce = Some("add-row".into());
        let b = self.add_caption(
            "",
            field,
            true,
            [x + label_w, y, x + label_w + value_w, y + h],
        );
        self.coalesce = None;
        (a, b)
    }

    fn default_link(&self) -> u32 {
        // 文字項目は写真が 2 枚以上あるときだけ所属写真（1 始まり）を持つ（サンプルの傾向）
        let pictures = self
            .template
            .objects
            .iter()
            .filter(|o| o.kind == KIND_PICTURE)
            .count();
        if pictures >= 2 {
            self.template
                .objects
                .iter()
                .rev()
                .find(|o| o.kind == KIND_CAPTION)
                .map(|o| o.link)
                .unwrap_or(1)
        } else {
            0
        }
    }

    fn push(&mut self, o: Object) -> usize {
        self.template.objects.push(o);
        let i = self.template.objects.len() - 1;
        self.selection = [i].into();
        i
    }

    pub fn delete_selected(&mut self) -> usize {
        let del: BTreeSet<usize> = self.selected_editable().into_iter().collect();
        if del.is_empty() {
            return 0;
        }
        self.checkpoint(None);
        let mut i = 0;
        self.template.objects.retain(|_| {
            let keep = !del.contains(&i);
            i += 1;
            keep
        });
        self.selection.clear();
        del.len()
    }

    pub fn copy_selected(&self) -> Vec<Object> {
        self.selected_editable()
            .into_iter()
            .map(|i| self.template.objects[i].clone())
            .collect()
    }

    pub fn paste(&mut self, objs: &[Object], offset: f64) {
        if objs.is_empty() {
            return;
        }
        self.checkpoint(None);
        let start = self.template.objects.len();
        for o in objs {
            let mut o = o.clone();
            for (k, v) in o.rect.iter_mut().enumerate() {
                let _ = k;
                *v += offset;
            }
            self.template.objects.push(o);
        }
        self.selection = (start..self.template.objects.len()).collect();
    }

    // ----- 位置・大きさ -----

    pub fn move_selected(&mut self, dx: f64, dy: f64, key: Option<&str>) {
        let sel = self.selected_editable();
        if sel.is_empty() {
            return;
        }
        self.checkpoint(key);
        for i in sel {
            let r = &mut self.template.objects[i].rect;
            r[0] += dx;
            r[2] += dx;
            r[1] += dy;
            r[3] += dy;
        }
    }

    pub fn bounds(&self, idx: &[usize]) -> Option<[f64; 4]> {
        let mut it = idx.iter().map(|&i| self.template.objects[i].normalized());
        let first = it.next()?;
        Some(it.fold(first, |a, b| {
            [
                a[0].min(b[0]),
                a[1].min(b[1]),
                a[2].max(b[2]),
                a[3].max(b[3]),
            ]
        }))
    }

    pub fn align(&mut self, how: Align) {
        let sel = self.selected_editable();
        if sel.len() < 2 && !matches!(how, Align::CardCenterH | Align::CardCenterV) {
            return;
        }
        let Some(b) = self.bounds(&sel) else { return };
        let frame = crate::layout::card_frame(&self.template);
        self.checkpoint(None);
        for &i in &sel {
            let n = self.template.objects[i].normalized();
            let (w, h) = (n[2] - n[0], n[3] - n[1]);
            let (dx, dy) = match how {
                Align::Left => (b[0] - n[0], 0.0),
                Align::CenterH => ((b[0] + b[2]) / 2.0 - (n[0] + w / 2.0), 0.0),
                Align::Right => (b[2] - n[2], 0.0),
                Align::Top => (0.0, b[1] - n[1]),
                Align::CenterV => (0.0, (b[1] + b[3]) / 2.0 - (n[1] + h / 2.0)),
                Align::Bottom => (0.0, b[3] - n[3]),
                Align::CardCenterH => (frame.width / 2.0 - (n[0] + w / 2.0), 0.0),
                Align::CardCenterV => (0.0, frame.height / 2.0 - (n[1] + h / 2.0)),
            };
            let r = &mut self.template.objects[i].rect;
            r[0] += dx;
            r[2] += dx;
            r[1] += dy;
            r[3] += dy;
        }
    }

    /// 間隔を等しくする（横 / 縦）
    pub fn distribute(&mut self, horizontal: bool) {
        let mut sel = self.selected_editable();
        if sel.len() < 3 {
            return;
        }
        let a = if horizontal { 0 } else { 1 };
        sel.sort_by(|&x, &y| {
            let nx = self.template.objects[x].normalized();
            let ny = self.template.objects[y].normalized();
            nx[a]
                .partial_cmp(&ny[a])
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let ns: Vec<[f64; 4]> = sel
            .iter()
            .map(|&i| self.template.objects[i].normalized())
            .collect();
        let total: f64 = ns.iter().map(|n| n[a + 2] - n[a]).sum();
        let span = ns.last().unwrap()[a + 2] - ns[0][a];
        let gap = (span - total) / (ns.len() - 1) as f64;
        self.checkpoint(None);
        let mut pos = ns[0][a];
        for (k, &i) in sel.iter().enumerate() {
            let d = pos - ns[k][a];
            let r = &mut self.template.objects[i].rect;
            r[a] += d;
            r[a + 2] += d;
            pos += ns[k][a + 2] - ns[k][a] + gap;
        }
    }

    /// 幅・高さを最初に選んだ物に揃える
    pub fn match_size(&mut self, width: bool, height: bool) {
        let sel = self.selected_editable();
        if sel.len() < 2 {
            return;
        }
        let n0 = self.template.objects[sel[0]].normalized();
        self.checkpoint(None);
        for &i in &sel[1..] {
            let n = self.template.objects[i].normalized();
            let r = &mut self.template.objects[i].rect;
            *r = [
                n[0],
                n[1],
                if width { n[0] + (n0[2] - n0[0]) } else { n[2] },
                if height { n[1] + (n0[3] - n0[1]) } else { n[3] },
            ];
        }
    }

    /// 重なり順の変更（最前面 = 末尾に移す）
    pub fn reorder(&mut self, to_front: bool) {
        let sel = self.selected_editable();
        if sel.is_empty() {
            return;
        }
        self.checkpoint(None);
        let set: BTreeSet<usize> = sel.into_iter().collect();
        let (picked, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.template.objects)
            .into_iter()
            .enumerate()
            .partition(|(i, _)| set.contains(i));
        let picked: Vec<Object> = picked.into_iter().map(|(_, o)| o).collect();
        let rest: Vec<Object> = rest.into_iter().map(|(_, o)| o).collect();
        let n = picked.len();
        if to_front {
            let start = rest.len();
            self.template.objects = rest.into_iter().chain(picked).collect();
            self.selection = (start..start + n).collect();
        } else {
            self.template.objects = picked.into_iter().chain(rest).collect();
            self.selection = (0..n).collect();
        }
    }

    /// カードの分割数・向きを変える（物の位置はそのまま）
    pub fn set_layout(&mut self, cols: u16, rows: u16, portrait: bool) {
        let h = &self.template.header;
        if h.cols == cols && h.rows == rows && h.is_portrait() == portrait {
            return;
        }
        self.checkpoint(None);
        let h = &mut self.template.header;
        h.cols = cols;
        h.rows = rows;
        h.orientation = portrait as u32;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    CenterH,
    Right,
    Top,
    CenterV,
    Bottom,
    CardCenterH,
    CardCenterV,
}

// ---------------------------------------------------------------------------
// テンプレートの置き場（本体のエクスポート形式: TMPEXP\expinf, TMPEXP\TEMP1x3_V\名前.TPL）

#[derive(Clone, Debug)]
pub struct LibraryEntry {
    pub folder: String,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Default)]
pub struct Library {
    pub root: PathBuf,
    pub entries: Vec<LibraryEntry>,
    /// expinf があり中身が本体の書き出しと同じか
    pub is_export: bool,
}

impl Library {
    pub fn scan(root: &Path) -> Result<Self, String> {
        let mut entries = Vec::new();
        let rd =
            std::fs::read_dir(root).map_err(|e| format!("{} を開けません: {e}", root.display()))?;
        for sub in rd.flatten() {
            let p = sub.path();
            let Some(folder) = p.file_name().map(|s| s.to_string_lossy().into_owned()) else {
                continue;
            };
            if !p.is_dir() || tpl::parse_folder_name(&folder).is_none() {
                continue;
            }
            let Ok(files) = std::fs::read_dir(&p) else {
                continue;
            };
            for f in files.flatten() {
                let fp = f.path();
                if fp
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("tpl"))
                {
                    let name = fp
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    entries.push(LibraryEntry {
                        folder: folder.clone(),
                        name,
                        path: fp,
                    });
                }
            }
        }
        entries.sort_by(|a, b| a.folder.cmp(&b.folder).then_with(|| a.name.cmp(&b.name)));
        let is_export =
            std::fs::read(root.join("expinf")).is_ok_and(|d| d.starts_with(b"Ver7.10Template"));
        Ok(Self {
            root: root.to_path_buf(),
            entries,
            is_export,
        })
    }

    /// この置き場の中で、テンプレートを保存する場所
    pub fn path_for(&self, header: &Header, name: &str) -> PathBuf {
        self.root
            .join(header.folder_name())
            .join(format!("{name}.TPL"))
    }
}

/// 本体の「テンプレートのインポート」で取り込める形で書き出す。
/// `dest` の下に TMPEXP フォルダを作り（既にあれば足す）、expinf とレイアウト別フォルダを置く。
/// 既にある同名ファイルは `overwrite` が真のときだけ置き換える。書いたファイルを返す。
pub fn export_templates(
    dest: &Path,
    items: &[(String, Template)],
    overwrite: bool,
) -> Result<Vec<PathBuf>, String> {
    let root = if dest
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("TMPEXP"))
    {
        dest.to_path_buf()
    } else {
        dest.join("TMPEXP")
    };
    std::fs::create_dir_all(&root).map_err(|e| format!("{} を作れません: {e}", root.display()))?;
    let inf = root.join("expinf");
    if !inf.exists() {
        std::fs::write(&inf, EXPINF_TEXT)
            .map_err(|e| format!("{} を書けません: {e}", inf.display()))?;
    }
    let mut written = Vec::new();
    for (name, t) in items {
        validate_name(name)?;
        let dir = root.join(t.header.folder_name());
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("{} を作れません: {e}", dir.display()))?;
        let path = dir.join(format!("{}.TPL", name.trim()));
        if path.exists() && !overwrite {
            return Err(format!(
                "{} と同名のテンプレートが既にあります。",
                name.trim()
            ));
        }
        let bytes = t.to_bytes();
        Template::parse(&bytes)
            .map_err(|e| format!("{name} の書き出し内容を確認できません（{e}）"))?;
        std::fs::write(&path, bytes)
            .map_err(|e| format!("{} を書けません: {e}", path.display()))?;
        written.push(path);
    }
    Ok(written)
}

/// 写真枠 1 つと、項目名・項目値の行を並べた最初の形（サンプルの 1x3 縦と同じ並び方）
pub fn starter_template(header: Header) -> Template {
    let mut doc = Document::new(
        Template {
            header,
            ..Default::default()
        },
        "",
        None,
    );
    let frame = crate::layout::card_frame(&doc.template);
    let (w, h) = (frame.width, frame.height);
    let m = 0.1;
    let photo_w = (w * 0.56).min(h * 1.33);
    doc.add_picture([m, m, m + photo_w, h - m]);
    let x = m + photo_w + 0.1;
    let label_w = ((w - x - m) * 0.32).max(0.5);
    let value_w = (w - x - m - label_w).max(0.5);
    let fields = ["工種", "種別", "写真タイトル", "撮影箇所", "撮影年月日"];
    let row_h = 0.25_f64.min((h - 2.0 * m) / fields.len() as f64);
    for (k, f) in fields.iter().enumerate() {
        doc.add_field_row(f, (x, m + k as f64 * row_h), label_w, value_w, row_h);
    }
    doc.template
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn doc_with(n: usize) -> Document {
        let mut d = Document::new(Template::default(), "t", None);
        for k in 0..n {
            let x = k as f64;
            d.add_caption("a", "", false, [x, 0.0, x + 0.5, 0.2 + k as f64 * 0.1]);
        }
        d
    }

    /// 履歴も未保存の印も選択も無い状態から始める
    fn clean_doc(n: usize) -> Document {
        let mut d = doc_with(n);
        d.undo.clear();
        d.redo.clear();
        d.coalesce = None;
        d.dirty = false;
        d.selection.clear();
        d
    }

    #[test]
    fn names_are_validated() {
        let cases = [
            ("工事写真", true),
            ("", false),
            ("a/b", false),
            ("x?y", false),
            ("  ", false),
        ];
        for (name, ok) in cases {
            assert_eq!(validate_name(name).is_ok(), ok, "{name:?}");
        }
    }

    #[test]
    fn undo_redo_restores_state() {
        let mut d = doc_with(2);
        let before = d.template.clone();
        d.select_all();
        d.move_selected(1.0, 1.0, None);
        assert_ne!(d.template, before);
        d.undo();
        assert_eq!(d.template, before);
        d.redo();
        assert_ne!(d.template, before);
    }

    #[test]
    fn coalesced_moves_make_one_undo_step() {
        let mut d = doc_with(1);
        let before = d.template.clone();
        d.select_all();
        for _ in 0..5 {
            d.move_selected(0.1, 0.0, Some("nudge"));
        }
        d.undo();
        assert_eq!(d.template, before);
    }

    #[test]
    fn align_and_distribute() {
        let cases = [
            (Align::Left, 0usize, 0.0),
            (Align::Right, 2, 2.5),
            (Align::Top, 1, 0.0),
        ];
        for (how, edge, expected) in cases {
            let mut d = doc_with(3);
            d.select_all();
            d.align(how);
            for o in &d.template.objects {
                assert!((o.normalized()[edge] - expected).abs() < 1e-9, "{how:?}");
            }
        }
        let mut d = doc_with(3);
        d.template.objects[1].rect = [0.6, 0.0, 1.1, 0.2];
        d.select_all();
        d.distribute(true);
        let n: Vec<_> = d.template.objects.iter().map(Object::normalized).collect();
        let g1 = n[1][0] - n[0][2];
        let g2 = n[2][0] - n[1][2];
        assert!((g1 - g2).abs() < 1e-9);
    }

    #[test]
    fn reorder_moves_selection() {
        let mut d = doc_with(3);
        d.selection = [0].into();
        let first = d.template.objects[0].clone();
        d.reorder(true);
        assert_eq!(d.template.objects[2], first);
        assert_eq!(d.selection, [2].into());
    }

    #[test]
    fn passthrough_objects_are_not_edited() {
        let mut d = doc_with(1);
        d.template.objects.push(Object {
            tag_version: 3,
            kind: tpl::KIND_RECT,
            link: 0,
            rect: [0.0, 0.0, 1.0, 1.0],
            body: Body::Passthrough(vec![0; 81]),
            trailer: Some(0),
        });
        let raw_before = d.template.objects[1].clone();
        d.select_all();
        d.move_selected(1.0, 1.0, None);
        assert_eq!(d.template.objects[1], raw_before);
        assert_eq!(d.delete_selected(), 1);
        assert_eq!(d.template.objects, vec![raw_before]);
    }

    #[test]
    fn export_creates_import_layout() {
        let dir = std::env::temp_dir().join(format!("cts-export-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let t = starter_template(Header::default());
        let written = export_templates(&dir, &[("見本".into(), t.clone())], false).expect("export");
        assert_eq!(written.len(), 1);
        assert!(written[0].ends_with("TMPEXP/TEMP1x3_V/見本.TPL"));
        assert_eq!(
            std::fs::read(dir.join("TMPEXP/expinf")).unwrap(),
            EXPINF_TEXT
        );
        assert_eq!(
            Template::parse(&std::fs::read(&written[0]).unwrap()).unwrap(),
            t
        );
        assert!(export_templates(&dir, &[("見本".into(), t.clone())], false).is_err());
        assert!(export_templates(&dir, &[("見本".into(), t)], true).is_ok());
        let lib = Library::scan(&dir.join("TMPEXP")).unwrap();
        assert!(lib.is_export);
        assert_eq!(lib.entries.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ----- 名前 -----

    #[test]
    fn name_validation_messages() {
        let long = "あ".repeat(101);
        let ok_long = "あ".repeat(100);
        let cases: Vec<(&str, Option<&str>)> = vec![
            ("工事写真", None),
            ("  前後に空白  ", None),
            (ok_long.as_str(), None),
            ("", Some("入力してください")),
            ("\t", Some("入力してください")),
            ("a\\b", Some("使用できない文字")),
            ("a|b", Some("使用できない文字")),
            ("a\nb", Some("使用できない文字")),
            ("😀", Some("扱えない文字")),
            (long.as_str(), Some("長すぎ")),
        ];
        for (name, err) in cases {
            match (validate_name(name), err) {
                (Ok(()), None) => {}
                (Err(e), Some(part)) => assert!(e.contains(part), "{name:?}: {e}"),
                (got, want) => panic!("{name:?}: {got:?} / {want:?}"),
            }
        }
    }

    // ----- 元に戻す -----

    #[test]
    fn new_change_clears_redo_and_caps_history() {
        let mut d = doc_with(1);
        d.select_all();
        d.move_selected(1.0, 0.0, None);
        d.undo();
        assert!(d.can_redo());
        d.move_selected(2.0, 0.0, None);
        assert!(!d.can_redo(), "新しい操作でやり直しは消える");
        for _ in 0..250 {
            d.move_selected(0.01, 0.0, None);
        }
        let mut n = 0;
        while d.can_undo() {
            d.undo();
            n += 1;
        }
        assert_eq!(n, 200, "履歴は 200 段まで");
    }

    #[test]
    fn undo_and_redo_on_empty_history_do_nothing() {
        let mut d = clean_doc(1);
        let before = d.template.clone();
        d.undo();
        d.redo();
        assert_eq!(d.template, before);
        assert!(!d.dirty);
        assert!(!d.can_undo() && !d.can_redo());
    }

    #[test]
    fn undo_restores_selection_too() {
        let mut d = doc_with(3);
        d.selection = [0, 2].into();
        d.move_selected(1.0, 0.0, None);
        d.selection = [1].into();
        d.undo();
        assert_eq!(d.selection, [0, 2].into());
        d.redo();
        assert_eq!(d.selection, [1].into());
    }

    #[test]
    fn coalesce_key_changes_start_new_steps() {
        let mut d = clean_doc(1);
        d.select_all();
        d.move_selected(0.1, 0.0, Some("a"));
        d.move_selected(0.1, 0.0, Some("a"));
        d.move_selected(0.1, 0.0, Some("b"));
        d.end_coalesce();
        d.move_selected(0.1, 0.0, Some("b"));
        let mut n = 0;
        while d.can_undo() {
            d.undo();
            n += 1;
        }
        assert_eq!(n, 3);
    }

    // ----- 追加・削除・複製 -----

    #[test]
    fn add_picture_inherits_from_existing_picture_but_clears_paths() {
        let mut d = Document::new(Template::default(), "t", None);
        let mut p = PictureBody {
            frame_style: 0x99,
            guid: SjisString::from_str("guid"),
            path: SjisString::from_str("C:\\a.jpg"),
            ..PictureBody::default()
        };
        p.e8 = 7;
        d.template
            .objects
            .push(Object::new(Body::Picture(p), [0.0, 0.0, 1.0, 1.0]));
        let i = d.add_picture([2.0, 0.0, 3.0, 1.0]);
        assert_eq!(d.selection, [i].into());
        match &d.template.objects[i].body {
            Body::Picture(p) => {
                assert_eq!((p.frame_style, p.e8), (0x99, 7), "未解明の値は引き継ぐ");
                assert!(p.guid.is_empty() && p.path.is_empty());
            }
            _ => unreachable!(),
        }
        assert!(d.dirty);
    }

    #[test]
    fn add_caption_inherits_format_of_same_role() {
        let mut d = Document::new(Template::default(), "t", None);
        let mut label = CaptionBody::new("固定", "", false, 20);
        label.border = 0;
        d.template
            .objects
            .push(Object::new(Body::Caption(label), [0.0, 0.0, 1.0, 1.0]));
        let a = d.add_caption("新しい項目名", "工種", false, [0.0, 1.0, 1.0, 2.0]);
        let b = d.add_caption("", "工種", true, [1.0, 1.0, 2.0, 2.0]);
        let get = |i: usize| match &d.template.objects[i].body {
            Body::Caption(c) => c.clone(),
            _ => unreachable!(),
        };
        let (ca, cb) = (get(a), get(b));
        assert_eq!(ca.border, 0, "同じ役割の物から書式を引き継ぐ");
        assert_eq!(ca.text.to_string_lossy(), "新しい項目名");
        assert_eq!(ca.value.to_string_lossy(), "0");
        assert_eq!(cb.border, 1, "項目値は既定の書式");
        assert_eq!(cb.is_value, 1);
        assert_eq!(tpl::read_fmt_u16(&cb.fmt, tpl::fmt_off::SIZE), 12);
    }

    #[test]
    fn add_caption_does_not_inherit_from_fixed_text() {
        // is_value = -1（固定文字）は項目名とも項目値とも別。新しい部品の書式の見本にしない
        let mut d = Document::new(Template::default(), "t", None);
        let mut fixed = CaptionBody::new("撮影日", "", false, 20);
        fixed.is_value = -1;
        fixed.t108 = -1;
        fixed.border = 0;
        d.template
            .objects
            .push(Object::new(Body::Caption(fixed), [0.0, 0.0, 1.0, 1.0]));
        let a = d.add_caption("項目名", "工種", false, [0.0, 1.0, 1.0, 2.0]);
        let b = d.add_caption("", "工種", true, [1.0, 1.0, 2.0, 2.0]);
        for (i, expect) in [(a, 0), (b, 1)] {
            let Body::Caption(c) = &d.template.objects[i].body else {
                unreachable!()
            };
            assert_eq!((c.is_value, c.t108, c.border), (expect, 0, 1));
        }
    }

    #[test]
    fn caption_link_follows_picture_count() {
        let mut d = Document::new(Template::default(), "t", None);
        d.add_picture([0.0, 0.0, 1.0, 1.0]);
        let a = d.add_caption("a", "", false, [0.0; 4]);
        assert_eq!(d.template.objects[a].link, 0, "写真 1 枚なら所属なし");
        d.add_picture([2.0, 0.0, 3.0, 1.0]);
        // 写真が 2 枚以上あるとき: 直前の文字項目の所属を引き継ぐ（それが 0 なら 0 のまま）
        let b = d.add_caption("b", "", false, [0.0; 4]);
        assert_eq!(d.template.objects[b].link, 0);
        d.template.objects[b].link = 2;
        let c = d.add_caption("c", "", false, [0.0; 4]);
        assert_eq!(d.template.objects[c].link, 2);
        // 文字項目がまだ無ければ 1 枚目の写真
        let mut d = Document::new(Template::default(), "t", None);
        d.add_picture([0.0, 0.0, 1.0, 1.0]);
        d.add_picture([2.0, 0.0, 3.0, 1.0]);
        let first = d.add_caption("a", "", false, [0.0; 4]);
        assert_eq!(d.template.objects[first].link, 1);
    }

    #[test]
    fn field_row_adds_label_then_value() {
        let mut d = Document::new(Template::default(), "t", None);
        let (a, b) = d.add_field_row("工種", (1.0, 1.0), 0.5, 1.5, 0.25);
        assert_eq!((a, b), (0, 1));
        assert_eq!(d.template.objects[0].rect, [1.0, 1.0, 1.5, 1.25]);
        assert_eq!(d.template.objects[1].rect, [1.5, 1.0, 3.0, 1.25]);
        assert_eq!(d.selection, [1].into());
        // 現状の挙動の固定: 名前と値は「元に戻す」2 段になる（1 段にまとめる意図の
        // coalesce 印は、checkpoint(None) が見ないため効いていない）
        d.undo();
        assert_eq!(d.template.objects.len(), 1);
        d.undo();
        assert!(d.template.objects.is_empty());
    }

    #[test]
    fn delete_copy_and_paste() {
        let mut d = doc_with(3);
        d.selection = [0, 2].into();
        let copied = d.copy_selected();
        assert_eq!(copied.len(), 2);
        assert_eq!(d.delete_selected(), 2);
        assert_eq!(d.template.objects.len(), 1);
        assert!(d.selection.is_empty());
        assert_eq!(d.delete_selected(), 0, "選択なしでは何も消えない");
        let before = d.template.objects.len();
        d.paste(&[], 0.1);
        assert_eq!(d.template.objects.len(), before, "空の貼り付けは何もしない");
        d.paste(&copied, 0.5);
        assert_eq!(d.template.objects.len(), 3);
        assert_eq!(d.selection, [1, 2].into());
        let r = d.template.objects[1].rect;
        assert!((r[0] - (copied[0].rect[0] + 0.5)).abs() < 1e-9);
        assert!((r[3] - (copied[0].rect[3] + 0.5)).abs() < 1e-9);
    }

    #[test]
    fn move_selected_with_nothing_selected_is_noop() {
        let mut d = clean_doc(2);
        d.move_selected(1.0, 1.0, None);
        assert!(!d.dirty && !d.can_undo());
    }

    // ----- 配置 -----

    #[test]
    fn align_every_direction() {
        // 3 個: x = 0, 1, 2 / 幅 0.5 / 高さ 0.2, 0.3, 0.4
        let cases = [
            (Align::Left, 0usize, 0.0),
            (Align::Right, 2, 2.5),
            (Align::Top, 1, 0.0),
            (Align::Bottom, 3, 0.4),
        ];
        for (how, edge, expected) in cases {
            let mut d = doc_with(3);
            d.select_all();
            d.align(how);
            for o in &d.template.objects {
                assert!((o.normalized()[edge] - expected).abs() < 1e-9, "{how:?}");
            }
        }
        let mut d = doc_with(3);
        d.select_all();
        d.align(Align::CenterH);
        for o in &d.template.objects {
            let n = o.normalized();
            assert!(((n[0] + n[2]) / 2.0 - 1.25).abs() < 1e-9);
        }
        d.align(Align::CenterV);
        for o in &d.template.objects {
            let n = o.normalized();
            assert!(((n[1] + n[3]) / 2.0 - 0.2).abs() < 1e-9);
        }
    }

    #[test]
    fn align_to_card_center_works_for_a_single_object() {
        let mut d = clean_doc(1);
        d.select_all();
        let frame = crate::layout::card_frame(&d.template);
        d.align(Align::CardCenterH);
        d.align(Align::CardCenterV);
        let n = d.template.objects[0].normalized();
        assert!(((n[0] + n[2]) / 2.0 - frame.width / 2.0).abs() < 1e-9);
        assert!(((n[1] + n[3]) / 2.0 - frame.height / 2.0).abs() < 1e-9);
        // 1 個では他の揃えは何もしない
        let mut d = clean_doc(1);
        d.select_all();
        d.align(Align::Left);
        assert!(!d.dirty);
    }

    #[test]
    fn distribute_needs_three_and_works_vertically() {
        let mut d = clean_doc(2);
        d.select_all();
        d.distribute(true);
        assert!(!d.dirty, "2 個では何もしない");
        let mut d = doc_with(3);
        d.template.objects[1].rect = [0.0, 0.5, 0.5, 0.6];
        d.template.objects[2].rect = [0.0, 2.0, 0.5, 2.3];
        d.template.objects[0].rect = [0.0, 0.0, 0.5, 0.2];
        d.select_all();
        d.distribute(false);
        let n: Vec<_> = d.template.objects.iter().map(Object::normalized).collect();
        assert!(((n[1][1] - n[0][3]) - (n[2][1] - n[1][3])).abs() < 1e-9);
    }

    #[test]
    fn match_size_uses_first_selected() {
        let mut d = doc_with(3);
        d.template.objects[0].rect = [0.0, 0.0, 2.0, 1.0];
        d.selection = [0, 1, 2].into();
        d.match_size(true, false);
        for o in &d.template.objects {
            let n = o.normalized();
            assert!((n[2] - n[0] - 2.0).abs() < 1e-9);
        }
        assert!(
            (d.template.objects[1].normalized()[3] - 0.3).abs() < 1e-9,
            "高さはそのまま"
        );
        d.match_size(false, true);
        assert!((d.template.objects[2].normalized()[3] - 1.0).abs() < 1e-9);
        let mut one = clean_doc(1);
        one.select_all();
        one.match_size(true, true);
        assert!(!one.dirty);
    }

    #[test]
    fn reorder_to_back_and_multi() {
        let mut d = doc_with(4);
        let ids: Vec<_> = d.template.objects.iter().map(|o| o.rect[0]).collect();
        d.selection = [1, 3].into();
        d.reorder(false);
        let order: Vec<_> = d.template.objects.iter().map(|o| o.rect[0]).collect();
        assert_eq!(order, vec![ids[1], ids[3], ids[0], ids[2]]);
        assert_eq!(d.selection, [0, 1].into());
        d.reorder(true);
        let order: Vec<_> = d.template.objects.iter().map(|o| o.rect[0]).collect();
        assert_eq!(order, vec![ids[0], ids[2], ids[1], ids[3]]);
        assert_eq!(d.selection, [2, 3].into());
        let mut none = clean_doc(2);
        none.reorder(true);
        assert!(!none.dirty);
    }

    #[test]
    fn set_layout_changes_header_once() {
        let mut d = doc_with(0);
        d.set_layout(1, 3, true);
        assert!(!d.dirty, "同じレイアウトなら何もしない");
        d.set_layout(2, 2, false);
        let h = &d.template.header;
        assert_eq!((h.cols, h.rows, h.is_portrait()), (2, 2, false));
        d.undo();
        assert_eq!(d.template.header.cols, 1);
    }

    #[test]
    fn selection_helpers() {
        let mut d = doc_with(2);
        d.template.objects.push(Object {
            tag_version: 3,
            kind: tpl::KIND_RECT,
            link: 0,
            rect: [0.0; 4],
            body: Body::Passthrough(vec![0; 81]),
            trailer: Some(0),
        });
        assert!(d.editable(0) && !d.editable(2) && !d.editable(99));
        d.select_all();
        assert_eq!(d.selection.len(), 3);
        assert_eq!(d.selected_editable(), vec![0, 1]);
        assert_eq!(d.bounds(&[]), None);
        let b = d.bounds(&[0, 1]).unwrap();
        assert_eq!(b[0], 0.0);
        assert_eq!(b[2], 1.5);
    }

    // ----- ファイル -----

    #[test]
    fn save_and_open_roundtrip() {
        let tmp = TempDir::new("doc-save");
        let path = tmp.path().join("下位").join("見本.TPL");
        let mut d = Document::new(starter_template(Header::default()), "新規", None);
        d.checkpoint(None);
        assert!(d.dirty);
        d.save_to(&path).expect("save");
        assert!(!d.dirty);
        assert_eq!(d.name, "見本");
        assert_eq!(d.path.as_deref(), Some(path.as_path()));
        let back = Document::open(&path).expect("open");
        assert_eq!(back.template, d.template);
        assert_eq!(back.name, "見本");
        assert_eq!(std::fs::read(&path).unwrap(), d.template.to_bytes());
    }

    #[test]
    fn open_reports_missing_and_broken_files() {
        let tmp = TempDir::new("doc-open");
        let missing = tmp.path().join("無い.TPL");
        let e = Document::open(&missing).err().expect("error");
        assert!(e.contains("読めません"), "{e}");
        let broken = tmp.path().join("壊れ.TPL");
        std::fs::write(&broken, b"not a template").unwrap();
        let e = Document::open(&broken).err().expect("error");
        assert!(e.contains("テンプレートとして読めません"), "{e}");
    }

    #[test]
    fn save_reports_unwritable_destination() {
        let tmp = TempDir::new("doc-save-err");
        let blocker = tmp.path().join("file");
        std::fs::write(&blocker, b"x").unwrap();
        let mut d = Document::new(Template::default(), "t", None);
        d.checkpoint(None);
        // ファイルの下にはフォルダを作れない
        let e = d.save_to(&blocker.join("x.TPL")).unwrap_err();
        assert!(e.contains("作れません"), "{e}");
        assert!(d.dirty, "失敗したら未保存のまま");
        assert!(d.path.is_none());
    }

    // ----- 置き場 -----

    #[test]
    fn library_scan_lists_only_layout_folders_sorted() {
        let tmp = TempDir::new("doc-lib");
        let root = tmp.path();
        let t = Template::default().to_bytes();
        for (dir, file) in [
            ("TEMP1x3_V", "b.TPL"),
            ("TEMP1x3_V", "a.tpl"),
            ("TEMP2x1_H", "z.TPL"),
            ("TEMP1x3_V", "memo.txt"),
            ("misc", "ignored.TPL"),
        ] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
            std::fs::write(root.join(dir).join(file), &t).unwrap();
        }
        std::fs::write(root.join("TEMP1x1_V"), b"file, not folder").unwrap();
        let lib = Library::scan(root).unwrap();
        let names: Vec<_> = lib
            .entries
            .iter()
            .map(|e| format!("{}/{}", e.folder, e.name))
            .collect();
        assert_eq!(names, vec!["TEMP1x3_V/a", "TEMP1x3_V/b", "TEMP2x1_H/z"]);
        assert!(!lib.is_export, "expinf が無い");
        std::fs::write(root.join("expinf"), EXPINF_TEXT).unwrap();
        assert!(Library::scan(root).unwrap().is_export);
        std::fs::write(root.join("expinf"), b"other").unwrap();
        assert!(!Library::scan(root).unwrap().is_export);
    }

    #[test]
    fn library_scan_fails_for_missing_root() {
        let tmp = TempDir::new("doc-lib-missing");
        let e = Library::scan(&tmp.path().join("無い")).unwrap_err();
        assert!(e.contains("開けません"), "{e}");
    }

    #[test]
    fn library_path_follows_layout() {
        let lib = Library {
            root: PathBuf::from("R"),
            ..Library::default()
        };
        let h = Header {
            cols: 2,
            rows: 1,
            orientation: 0,
            ..Header::default()
        };
        assert_eq!(
            lib.path_for(&h, "名前"),
            PathBuf::from("R/TEMP2x1_H/名前.TPL")
        );
    }

    // ----- 書き出し -----

    #[test]
    fn export_accepts_tmpexp_folder_and_validates_names() {
        let tmp = TempDir::new("doc-export");
        let t = Template::default();
        // TMPEXP を直接指定すると、その中に書く（二重にしない）
        let root = tmp.path().join("tmpexp");
        let written = export_templates(&root, &[("a".into(), t.clone())], false).unwrap();
        assert!(written[0].starts_with(&root));
        assert!(!root.join("TMPEXP").exists());
        // 不正な名前は書き出し前に弾く
        let e = export_templates(tmp.path(), &[("a/b".into(), t.clone())], false).unwrap_err();
        assert!(e.contains("使用できない文字"), "{e}");
        // 名前の前後の空白は取り除く
        let w = export_templates(tmp.path(), &[("  x  ".into(), t.clone())], false).unwrap();
        assert!(w[0].ends_with("x.TPL"));
        // 同名は overwrite が無ければ拒否、複数件は同じ expinf を共有
        let e = export_templates(tmp.path(), &[("x".into(), t.clone())], false).unwrap_err();
        assert!(e.contains("同名"), "{e}");
        let items = vec![("p".to_string(), t.clone()), ("q".to_string(), t)];
        assert_eq!(export_templates(tmp.path(), &items, true).unwrap().len(), 2);
        assert_eq!(
            std::fs::read(tmp.path().join("TMPEXP/expinf")).unwrap(),
            EXPINF_TEXT
        );
    }

    #[test]
    fn export_keeps_existing_expinf() {
        let tmp = TempDir::new("doc-export-inf");
        let root = tmp.path().join("TMPEXP");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("expinf"), b"keep").unwrap();
        export_templates(tmp.path(), &[("a".into(), Template::default())], false).unwrap();
        assert_eq!(std::fs::read(root.join("expinf")).unwrap(), b"keep");
    }

    // ----- 見本 -----

    #[test]
    fn starter_template_layouts() {
        let layouts = [(1u16, 3u16, 1u32), (2, 2, 0), (5, 5, 1), (1, 1, 0)];
        for (cols, rows, orientation) in layouts {
            let header = Header {
                cols,
                rows,
                orientation,
                ..Header::default()
            };
            let t = starter_template(header);
            assert_eq!(t.objects.len(), 1 + 5 * 2, "{cols}x{rows}");
            assert_eq!(t.objects[0].kind, KIND_PICTURE);
            assert!(t.objects[1..].iter().all(|o| o.kind == KIND_CAPTION));
            let frame = crate::layout::card_frame(&t);
            for o in &t.objects {
                let n = o.normalized();
                assert!(n[0] >= 0.0 && n[1] >= 0.0, "{cols}x{rows} {n:?}");
                assert!(n[2] <= frame.width + 1e-9 && n[3] <= frame.height + 1e-9);
            }
            assert_eq!(Template::parse(&t.to_bytes()).unwrap(), t);
        }
    }

    #[test]
    fn known_fields_are_unique() {
        let set: BTreeSet<_> = KNOWN_FIELDS.iter().collect();
        assert_eq!(set.len(), KNOWN_FIELDS.len());
    }
}
