//! OS のダイアログ（ファイル・フォルダの選択、確認）。
//!
//! アプリ本体は `Dialogs` だけを知り、`rfd` には触れない。テストでは別の実装に差し替える。

use std::path::{Path, PathBuf};

pub(super) trait Dialogs {
    /// 開くテンプレートを選ぶ。`dir` は最初に見せるフォルダ。取り消しは `None`
    fn open_template(&self, dir: Option<&Path>) -> Option<PathBuf>;
    /// 保存先を選ぶ。`file_name` は最初に入れておく名前。取り消しは `None`
    fn save_template(&self, dir: Option<&Path>, file_name: &str) -> Option<PathBuf>;
    /// フォルダを選ぶ。取り消しは `None`
    fn pick_folder(&self, title: Option<&str>) -> Option<PathBuf>;
    /// 「はい / いいえ」の確認。はいなら `true`
    fn confirm(&self, title: &str, description: &str) -> bool;
}

/// 本体のダイアログ（`rfd`）
pub(super) struct NativeDialogs;

impl NativeDialogs {
    fn template_filter() -> rfd::FileDialog {
        rfd::FileDialog::new().add_filter("カードテンプレート", &["TPL", "tpl"])
    }
}

impl Dialogs for NativeDialogs {
    fn open_template(&self, dir: Option<&Path>) -> Option<PathBuf> {
        let mut dlg = Self::template_filter();
        if let Some(dir) = dir {
            dlg = dlg.set_directory(dir);
        }
        dlg.pick_file()
    }

    fn save_template(&self, dir: Option<&Path>, file_name: &str) -> Option<PathBuf> {
        let mut dlg = Self::template_filter().set_file_name(file_name);
        if let Some(dir) = dir {
            dlg = dlg.set_directory(dir);
        }
        dlg.save_file()
    }

    fn pick_folder(&self, title: Option<&str>) -> Option<PathBuf> {
        let mut dlg = rfd::FileDialog::new();
        if let Some(title) = title {
            dlg = dlg.set_title(title);
        }
        dlg.pick_folder()
    }

    fn confirm(&self, title: &str, description: &str) -> bool {
        rfd::MessageDialog::new()
            .set_title(title)
            .set_description(description)
            .set_buttons(rfd::MessageButtons::YesNo)
            .show()
            == rfd::MessageDialogResult::Yes
    }
}
