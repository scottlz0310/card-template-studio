//! ファイル操作：開く・保存・書き出しと、ダイアログを使う操作。

use super::*;

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
        let mut dlg = rfd::FileDialog::new()
            .add_filter("カードテンプレート", &["TPL", "tpl"])
            .set_file_name(format!("{}.TPL", self.doc.name));
        if let Some(lib) = &self.library {
            dlg = dlg.set_directory(lib.root.join(self.doc.template.header.folder_name()));
        }
        if let Some(p) = dlg.save_file() {
            self.save_to(p, ctx);
        }
    }

    pub(super) fn export_dialog(&mut self, ctx: &egui::Context, all: bool) {
        let Some(dest) = rfd::FileDialog::new()
            .set_title("書き出し先のフォルダを選んでください（中に TMPEXP フォルダを作ります）")
            .pick_folder()
        else {
            return;
        };
        let mut items: Vec<(String, Template)> = Vec::new();
        if all && let Some(lib) = &self.library {
            for e in &lib.entries {
                if self.doc.path.as_deref() == Some(e.path.as_path()) {
                    items.push((self.doc.name.clone(), self.doc.template.clone()));
                    continue;
                }
                match std::fs::read(&e.path)
                    .map_err(|x| x.to_string())
                    .and_then(|d| Template::parse(&d).map_err(|x| x.to_string()))
                {
                    Ok(t) => items.push((e.name.clone(), t)),
                    Err(err) => {
                        self.error(
                            "書き出せません",
                            format!("{} を読めません: {err}", e.path.display()),
                        );
                        return;
                    }
                }
            }
        } else {
            items.push((self.doc.name.clone(), self.doc.template.clone()));
        }
        let mut result = doc::export_templates(&dest, &items, false);
        if let Err(e) = &result
            && e.contains("同名")
            && rfd::MessageDialog::new()
                .set_title("同名のテンプレート")
                .set_description(format!("{e}\n上書きしてもよろしいですか？"))
                .set_buttons(rfd::MessageButtons::YesNo)
                .show()
                == rfd::MessageDialogResult::Yes
        {
            result = doc::export_templates(&dest, &items, true);
        }
        match result {
            Ok(files) => self.toast(format!("テンプレートのエクスポートは正常に終了しました（{} 件）。本体の「テンプレートのインポート」で TMPEXP フォルダを選んでください。", files.len()), ctx),
            Err(e) => self.error("書き出せません", e),
        }
    }

    pub(super) fn open_dialog(&mut self) {
        let mut dlg = rfd::FileDialog::new().add_filter("カードテンプレート", &["TPL", "tpl"]);
        if let Some(lib) = &self.library {
            dlg = dlg.set_directory(&lib.root);
        }
        if let Some(p) = dlg.pick_file() {
            self.guard(Pending::Open(p));
        }
    }
}
