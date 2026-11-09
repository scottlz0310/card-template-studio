//! ファイル操作：開く・保存・書き出しと、ダイアログを使う操作。

use super::*;
use std::path::Path;

impl StudioApp {
    pub(super) fn open_file(&mut self, path: PathBuf) {
        match Document::open(&path) {
            Ok(d) => {
                self.doc = d;
                self.view.fit_pending = true;
                self.tool = Tool::Select;
            }
            Err(e) => self.error("開けませんでした", e),
        }
    }

    pub(super) fn open_library(&mut self, root: PathBuf) {
        match Library::scan(&root) {
            Ok(lib) if lib.entries.is_empty() => self.error(
                "テンプレートが見つかりません",
                format!("{} の中に「TEMP1x3_V」のようなフォルダと .TPL ファイルが見つかりませんでした。\n本体の「テンプレートのエクスポート」で作った TMPEXP フォルダを選んでください。", root.display()),
            ),
            Ok(lib) => self.library = Some(lib),
            Err(e) => self.error("フォルダを開けません", e),
        }
    }

    pub(super) fn save(&mut self, ctx: &egui::Context) {
        let target = self.doc.path.clone().or_else(|| {
            self.library
                .as_ref()
                .map(|l| l.path_for(&self.doc.template.header, &self.doc.name))
        });
        match target {
            Some(p) => self.save_to(p, ctx),
            None => {
                self.modal = Some(Modal::SaveAsName {
                    name: self.doc.name.clone(),
                    error: None,
                })
            }
        }
    }

    pub(super) fn save_to(&mut self, path: PathBuf, ctx: &egui::Context) {
        if let Err(e) = validate_name(
            &path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
        ) {
            self.error("保存できません", e);
            return;
        }
        match self.doc.save_to(&path) {
            Ok(()) => {
                self.toast(format!("保存しました: {}", path.display()), ctx);
                if let Some(lib) = &self.library
                    && path.starts_with(&lib.root)
                {
                    let root = lib.root.clone();
                    if let Ok(l) = Library::scan(&root) {
                        self.library = Some(l);
                    }
                }
            }
            Err(e) => self.error("保存できません", e),
        }
    }

    pub(super) fn save_as_dialog(&mut self, ctx: &egui::Context) {
        let dir = self
            .library
            .as_ref()
            .map(|lib| lib.root.join(self.doc.template.header.folder_name()));
        let file_name = format!("{}.TPL", self.doc.name);
        if let Some(p) = self.dialogs.save_template(dir.as_deref(), &file_name) {
            self.save_to(p, ctx);
        }
    }

    pub(super) fn export_dialog(&mut self, ctx: &egui::Context, all: bool) {
        let Some(dest) = self.dialogs.pick_folder(Some(
            "書き出し先のフォルダを選んでください（中に TMPEXP フォルダを作ります）",
        )) else {
            return;
        };
        let items = match export_items(&self.doc, self.library.as_ref(), all) {
            Ok(items) => items,
            Err(e) => {
                self.error("書き出せません", e);
                return;
            }
        };
        match export_with_confirm(&dest, &items, self.dialogs.as_ref()) {
            Ok(files) => self.toast(format!("テンプレートのエクスポートは正常に終了しました（{} 件）。本体の「テンプレートのインポート」で TMPEXP フォルダを選んでください。", files.len()), ctx),
            Err(e) => self.error("書き出せません", e),
        }
    }

    pub(super) fn open_dialog(&mut self) {
        let dir = self.library.as_ref().map(|lib| lib.root.clone());
        if let Some(p) = self.dialogs.open_template(dir.as_deref()) {
            self.guard(Pending::Open(p));
        }
    }

    /// フォルダを選んで、本体のエクスポート（TMPEXP）として開く
    pub(super) fn open_library_dialog(&mut self, title: Option<&str>) {
        if let Some(p) = self.dialogs.pick_folder(title) {
            self.open_library(p);
        }
    }
}

/// 書き出す項目（名前とテンプレート）を集める。まとめて書き出すときは一覧のファイルを読む。
/// 開いているテンプレートは、保存前の編集中の内容を使う。
pub(super) fn export_items(
    doc: &Document,
    library: Option<&Library>,
    all: bool,
) -> Result<Vec<(String, Template)>, String> {
    let mut items: Vec<(String, Template)> = Vec::new();
    if all && let Some(lib) = library {
        for e in &lib.entries {
            if doc.path.as_deref() == Some(e.path.as_path()) {
                items.push((doc.name.clone(), doc.template.clone()));
                continue;
            }
            match std::fs::read(&e.path)
                .map_err(|x| x.to_string())
                .and_then(|d| Template::parse(&d).map_err(|x| x.to_string()))
            {
                Ok(t) => items.push((e.name.clone(), t)),
                Err(err) => return Err(format!("{} を読めません: {err}", e.path.display())),
            }
        }
    } else {
        items.push((doc.name.clone(), doc.template.clone()));
    }
    Ok(items)
}

/// まず上書きなしで書き、同名があれば確認して、承諾されたら上書きで書き直す
pub(super) fn export_with_confirm(
    dest: &Path,
    items: &[(String, Template)],
    dialogs: &dyn Dialogs,
) -> Result<Vec<PathBuf>, String> {
    let mut result = doc::export_templates(dest, items, false);
    if let Err(e) = &result
        && e.contains("同名")
        && dialogs.confirm(
            "同名のテンプレート",
            &format!("{e}\n上書きしてもよろしいですか？"),
        )
    {
        result = doc::export_templates(dest, items, true);
    }
    result
}
