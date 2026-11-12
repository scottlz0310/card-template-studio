//! ダイアログを使う操作のテスト。`FakeDialogs` で答えを決め、渡された引数を記録して確かめる。

use super::dialogs::Dialogs;
use super::files::{export_items, export_with_confirm};
use super::*;
use crate::settings::memory::MemoryStore;
use crate::settings::{FileStore, Folders, Settings, SettingsStore, ViewSettings};
use crate::test_support::TempDir;
use crate::tpl::KIND_CAPTION;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::Path;
use std::rc::Rc;

/// 呼ばれたダイアログと、渡された引数
#[derive(Debug, PartialEq)]
enum Call {
    Open(Option<PathBuf>),
    Save(Option<PathBuf>, String),
    Folder(Option<String>, Option<PathBuf>),
    Confirm(String, String),
}

/// 答えを順に返す。答えが尽きたら取り消し（`None` / `false`）
#[derive(Default)]
struct FakeDialogs {
    open: RefCell<VecDeque<PathBuf>>,
    save: RefCell<VecDeque<PathBuf>>,
    folder: RefCell<VecDeque<PathBuf>>,
    confirm: RefCell<VecDeque<bool>>,
    calls: RefCell<Vec<Call>>,
}

impl FakeDialogs {
    fn calls(&self) -> Vec<Call> {
        std::mem::take(&mut *self.calls.borrow_mut())
    }
}

impl Dialogs for FakeDialogs {
    fn open_template(&self, dir: Option<&Path>) -> Option<PathBuf> {
        self.calls
            .borrow_mut()
            .push(Call::Open(dir.map(Path::to_path_buf)));
        self.open.borrow_mut().pop_front()
    }
    fn save_template(&self, dir: Option<&Path>, file_name: &str) -> Option<PathBuf> {
        self.calls
            .borrow_mut()
            .push(Call::Save(dir.map(Path::to_path_buf), file_name.to_owned()));
        self.save.borrow_mut().pop_front()
    }
    fn pick_folder(&self, title: Option<&str>, dir: Option<&Path>) -> Option<PathBuf> {
        self.calls.borrow_mut().push(Call::Folder(
            title.map(str::to_owned),
            dir.map(Path::to_path_buf),
        ));
        self.folder.borrow_mut().pop_front()
    }
    fn confirm(&self, title: &str, description: &str) -> bool {
        self.calls
            .borrow_mut()
            .push(Call::Confirm(title.to_owned(), description.to_owned()));
        self.confirm.borrow_mut().pop_front().unwrap_or(false)
    }
}

/// テストと app で同じ `FakeDialogs` を共有するための橋渡し
impl Dialogs for Rc<FakeDialogs> {
    fn open_template(&self, dir: Option<&Path>) -> Option<PathBuf> {
        (**self).open_template(dir)
    }
    fn save_template(&self, dir: Option<&Path>, file_name: &str) -> Option<PathBuf> {
        (**self).save_template(dir, file_name)
    }
    fn pick_folder(&self, title: Option<&str>, dir: Option<&Path>) -> Option<PathBuf> {
        (**self).pick_folder(title, dir)
    }
    fn confirm(&self, title: &str, description: &str) -> bool {
        (**self).confirm(title, description)
    }
}

type H = Harness<'static, StudioApp>;

fn harness(open: Option<PathBuf>) -> (H, Rc<FakeDialogs>) {
    harness_with_store(open, MemoryStore::default())
}

/// 設定の保存先を、テストが握っている `MemoryStore` にして起動する
fn harness_with_store(
    open: Option<PathBuf>,
    store: impl SettingsStore + 'static,
) -> (H, Rc<FakeDialogs>) {
    let fake = Rc::new(FakeDialogs::default());
    let shared = fake.clone();
    let mut h = Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .allow_missing_glyphs()
        .with_accessibility_check(true)
        .with_max_steps(30)
        .build_eframe(move |cc| {
            StudioApp::with_dialogs(&cc.egui_ctx, open, None, Box::new(shared), Box::new(store))
        });
    h.run();
    (h, fake)
}

fn app(h: &mut H) -> &mut StudioApp {
    h.state_mut()
}

fn press(h: &mut H, modifiers: Modifiers, key: Key) {
    h.key_press_modifiers(modifiers, key);
    h.run();
}

fn modal_title(h: &H) -> Option<String> {
    match &h.state().modal {
        Some(Modal::Message { title, .. }) => Some(title.clone()),
        _ => None,
    }
}

fn modal_body(h: &H) -> Option<String> {
    match &h.state().modal {
        Some(Modal::Message { body, .. }) => Some(body.clone()),
        _ => None,
    }
}

fn write_template(dir: &Path, folder: &str, name: &str) -> PathBuf {
    let path = dir.join(folder).join(format!("{name}.TPL"));
    let mut d = Document::new(doc::starter_template(Header::default()), name, None);
    d.save_to(&path).expect("save sample");
    path
}

fn open_menu(h: &mut H, menu: &str, item: &str) {
    h.get_all_by_label(menu).next().expect("メニュー").click();
    h.run();
    h.get_all_by_label_contains(item)
        .last()
        .expect("メニュー項目")
        .click();
    h.run();
}

const EXPORT_TITLE: &str = "書き出し先のフォルダを選んでください（中に TMPEXP フォルダを作ります）";

// ----- 開く -----

#[test]
fn open_dialog_passes_library_root_and_opens_the_chosen_file() {
    let tmp = TempDir::new("dlg-open");
    let file = write_template(tmp.path(), "TEMP1x3_V", "甲");

    // 一覧が無ければ、最初のフォルダの指定は無い
    let (mut h, fake) = harness(None);
    fake.open.borrow_mut().push_back(file.clone());
    app(&mut h).open_dialog();
    assert_eq!(fake.calls(), vec![Call::Open(None)]);
    assert_eq!(app(&mut h).doc.name, "甲");

    // 一覧があれば、その root を最初に見せる
    let (mut h, fake) = harness(Some(tmp.path().to_path_buf()));
    app(&mut h).open_dialog();
    assert_eq!(
        fake.calls(),
        vec![Call::Open(Some(tmp.path().to_path_buf()))]
    );
}

#[test]
fn open_dialog_cancel_changes_nothing_and_unsaved_changes_are_guarded() {
    let tmp = TempDir::new("dlg-open-cancel");
    let file = write_template(tmp.path(), "TEMP1x3_V", "甲");
    let (mut h, fake) = harness(None);
    app(&mut h).open_dialog();
    assert_eq!(app(&mut h).doc.name, "新規テンプレート", "取り消し");
    assert!(app(&mut h).modal.is_none());
    // 未保存の変更があれば、開く前に確認を挟む
    app(&mut h).doc.checkpoint(None);
    fake.open.borrow_mut().push_back(file);
    app(&mut h).open_dialog();
    assert!(matches!(app(&mut h).modal, Some(Modal::ConfirmDiscard(_))));
    assert_eq!(app(&mut h).doc.name, "新規テンプレート");
}

#[test]
fn ctrl_o_asks_for_a_file() {
    let (mut h, fake) = harness(None);
    press(&mut h, Modifiers::COMMAND, Key::O);
    assert_eq!(fake.calls(), vec![Call::Open(None)]);
}

// ----- 保存先 -----

#[test]
fn save_as_passes_default_name_and_layout_folder() {
    let tmp = TempDir::new("dlg-saveas");
    // 一覧が無い: フォルダの指定は無く、名前は「{名前}.TPL」
    let (mut h, fake) = harness(None);
    let ctx = h.ctx.clone();
    let path = tmp.path().join("別名.TPL");
    fake.save.borrow_mut().push_back(path.clone());
    app(&mut h).doc.checkpoint(None);
    app(&mut h).save_as_dialog(&ctx);
    assert_eq!(
        fake.calls(),
        vec![Call::Save(None, "新規テンプレート.TPL".into())]
    );
    assert!(path.exists());
    assert!(!app(&mut h).doc.dirty);
    assert_eq!(app(&mut h).doc.name, "別名");

    // 一覧がある: レイアウトのフォルダ（TEMP1x3_V）を最初に見せる
    write_template(tmp.path(), "TEMP1x3_V", "既存");
    let (mut h, fake) = harness(Some(tmp.path().to_path_buf()));
    app(&mut h).save_as_dialog(&ctx);
    assert_eq!(
        fake.calls(),
        vec![Call::Save(
            Some(tmp.path().join("TEMP1x3_V")),
            "新規テンプレート.TPL".into()
        )]
    );
}

#[test]
fn save_as_cancel_saves_nothing() {
    let (mut h, fake) = harness(None);
    let ctx = h.ctx.clone();
    app(&mut h).doc.checkpoint(None);
    app(&mut h).save_as_dialog(&ctx);
    assert_eq!(fake.calls().len(), 1);
    assert!(app(&mut h).doc.dirty && app(&mut h).doc.path.is_none());
    assert!(app(&mut h).toast.is_none());
}

#[test]
fn ctrl_shift_s_opens_the_save_as_dialog() {
    // Ctrl+Shift+S は Ctrl+S（上書き保存）に先取りされず、「名前を付けて保存」のダイアログが開く
    let (mut h, fake) = harness(None);
    press(&mut h, Modifiers::COMMAND | Modifiers::SHIFT, Key::S);
    assert_eq!(
        fake.calls(),
        vec![Call::Save(None, "新規テンプレート.TPL".into())]
    );
    assert!(app(&mut h).modal.is_none());
}

#[test]
fn name_dialog_goes_on_to_the_destination_dialog() {
    let tmp = TempDir::new("dlg-name");
    let path = tmp.path().join("決めた名前.TPL");
    let (mut h, fake) = harness(None);
    fake.save.borrow_mut().push_back(path.clone());
    app(&mut h).doc.checkpoint(None);
    app(&mut h).modal = Some(Modal::SaveAsName {
        name: " 決めた名前 ".into(),
        error: None,
    });
    h.run();
    h.get_by_label("保存先を選ぶ…").click();
    h.run();
    assert_eq!(
        fake.calls(),
        vec![Call::Save(None, "決めた名前.TPL".into())],
        "前後の空白は取り除いて渡す"
    );
    assert!(path.exists());
    assert_eq!(app(&mut h).doc.name, "決めた名前");
    assert!(app(&mut h).modal.is_none());
}

// ----- TMPEXP を開く -----

#[test]
fn menu_and_panel_buttons_pick_a_folder_with_their_own_titles() {
    let tmp = TempDir::new("dlg-folder");
    write_template(tmp.path(), "TEMP1x3_V", "甲");

    // メニュー: タイトルあり
    let (mut h, fake) = harness(None);
    fake.folder.borrow_mut().push_back(tmp.path().to_path_buf());
    open_menu(
        &mut h,
        "ファイル",
        "エクスポートしたフォルダ（TMPEXP）を開く",
    );
    assert_eq!(
        fake.calls(),
        vec![Call::Folder(
            Some("TMPEXP フォルダを選んでください".into()),
            None
        )]
    );
    assert_eq!(app(&mut h).library.as_ref().unwrap().entries.len(), 1);

    // 左パネルのボタン（一覧が無いとき）: タイトルなし
    let (mut h, fake) = harness(None);
    fake.folder.borrow_mut().push_back(tmp.path().to_path_buf());
    h.get_by_label("📂 TMPEXP フォルダを開く…").click();
    h.run();
    assert_eq!(fake.calls(), vec![Call::Folder(None, None)]);
    assert!(app(&mut h).library.is_some());
}

#[test]
fn cancelling_the_folder_dialog_keeps_the_state() {
    let (mut h, fake) = harness(None);
    h.get_by_label("📂 TMPEXP フォルダを開く…").click();
    h.run();
    assert_eq!(fake.calls(), vec![Call::Folder(None, None)]);
    assert!(app(&mut h).library.is_none() && app(&mut h).modal.is_none());
}

#[test]
fn an_empty_folder_is_reported_as_an_error() {
    let tmp = TempDir::new("dlg-empty");
    let (mut h, fake) = harness(None);
    fake.folder.borrow_mut().push_back(tmp.path().to_path_buf());
    h.get_by_label("📂 TMPEXP フォルダを開く…").click();
    h.run();
    assert_eq!(
        modal_title(&h).as_deref(),
        Some("テンプレートが見つかりません")
    );
}

// ----- 書き出し -----

#[test]
fn export_current_template_writes_a_tmpexp_folder() {
    let tmp = TempDir::new("dlg-export");
    let (mut h, fake) = harness(None);
    let ctx = h.ctx.clone();
    fake.folder.borrow_mut().push_back(tmp.path().to_path_buf());
    app(&mut h).export_dialog(&ctx, false);
    assert_eq!(
        fake.calls(),
        vec![Call::Folder(Some(EXPORT_TITLE.into()), None)]
    );
    assert!(
        tmp.path()
            .join("TMPEXP/TEMP1x3_V/新規テンプレート.TPL")
            .exists()
    );
    assert!(tmp.path().join("TMPEXP/expinf").exists());
    let toast = app(&mut h).toast.clone().expect("toast").0;
    assert!(toast.contains("（1 件）"), "{toast}");
}

#[test]
fn export_cancel_does_nothing() {
    let (mut h, fake) = harness(None);
    let ctx = h.ctx.clone();
    app(&mut h).export_dialog(&ctx, false);
    assert_eq!(fake.calls().len(), 1);
    assert!(app(&mut h).toast.is_none() && app(&mut h).modal.is_none());
}

#[test]
fn ctrl_e_and_the_menu_export_the_current_template() {
    let tmp = TempDir::new("dlg-export-keys");
    let (mut h, fake) = harness(None);
    fake.folder.borrow_mut().push_back(tmp.path().to_path_buf());
    press(&mut h, Modifiers::COMMAND, Key::E);
    assert_eq!(
        fake.calls(),
        vec![Call::Folder(Some(EXPORT_TITLE.into()), None)]
    );
    assert!(tmp.path().join("TMPEXP").exists());

    let (mut h, fake) = harness(None);
    open_menu(&mut h, "ファイル", "このテンプレートを本体用に書き出す");
    assert_eq!(
        fake.calls(),
        vec![Call::Folder(Some(EXPORT_TITLE.into()), None)]
    );
}

#[test]
fn export_all_writes_every_listed_template_using_the_open_one_as_edited() {
    let lib = TempDir::new("dlg-all-lib");
    let dest = TempDir::new("dlg-all-dest");
    let a = write_template(lib.path(), "TEMP1x3_V", "甲");
    write_template(lib.path(), "TEMP2x1_H", "乙");
    let (mut h, fake) = harness(Some(lib.path().to_path_buf()));
    let ctx = h.ctx.clone();
    // 甲を開いて、保存せずに編集する
    app(&mut h).open_file(a);
    app(&mut h).doc.select_all();
    app(&mut h).doc.delete_selected();
    fake.folder
        .borrow_mut()
        .push_back(dest.path().to_path_buf());
    app(&mut h).export_dialog(&ctx, true);
    let toast = app(&mut h).toast.clone().expect("toast").0;
    assert!(toast.contains("（2 件）"), "{toast}");
    let exported =
        Template::parse(&std::fs::read(dest.path().join("TMPEXP/TEMP1x3_V/甲.TPL")).unwrap())
            .unwrap();
    assert!(exported.objects.is_empty(), "開いている物は編集中の内容");
    // 書き出し先のフォルダはファイルの置き場所ではなく、中身のヘッダ（既定の 1x3 縦）で決まる
    assert!(dest.path().join("TMPEXP/TEMP1x3_V/乙.TPL").exists());
}

#[test]
fn export_all_reports_an_unreadable_file_and_writes_nothing() {
    let lib = TempDir::new("dlg-bad-lib");
    let dest = TempDir::new("dlg-bad-dest");
    write_template(lib.path(), "TEMP1x3_V", "甲");
    let (mut h, fake) = harness(Some(lib.path().to_path_buf()));
    let ctx = h.ctx.clone();
    std::fs::write(lib.path().join("TEMP1x3_V/壊れ.TPL"), b"broken").unwrap();
    app(&mut h).open_library(lib.path().to_path_buf());
    fake.folder
        .borrow_mut()
        .push_back(dest.path().to_path_buf());
    app(&mut h).export_dialog(&ctx, true);
    assert_eq!(modal_title(&h).as_deref(), Some("書き出せません"));
    assert!(modal_body(&h).unwrap().contains("壊れ.TPL を読めません"));
    assert!(!dest.path().join("TMPEXP").exists(), "何も書かない");
}

#[test]
fn export_asks_before_overwriting_a_same_named_template() {
    // 承諾: 上書きして成功
    let dest = TempDir::new("dlg-over-yes");
    let (mut h, fake) = harness(None);
    let ctx = h.ctx.clone();
    for _ in 0..2 {
        fake.folder
            .borrow_mut()
            .push_back(dest.path().to_path_buf());
    }
    app(&mut h).export_dialog(&ctx, false);
    assert_eq!(fake.calls().len(), 1, "初回は確認なし");
    app(&mut h).doc.template.card_color = 0x0012_3456;
    fake.confirm.borrow_mut().push_back(true);
    app(&mut h).export_dialog(&ctx, false);
    let calls = fake.calls();
    assert_eq!(calls.len(), 2);
    match &calls[1] {
        Call::Confirm(title, description) => {
            assert_eq!(title, "同名のテンプレート");
            assert!(description.contains("同名"), "{description}");
            assert!(description.ends_with("\n上書きしてもよろしいですか？"));
        }
        other => panic!("確認のはず: {other:?}"),
    }
    let written = Template::parse(
        &std::fs::read(dest.path().join("TMPEXP/TEMP1x3_V/新規テンプレート.TPL")).unwrap(),
    )
    .unwrap();
    assert_eq!(written.card_color, 0x0012_3456);
    assert!(app(&mut h).modal.is_none());

    // 拒否: 上書きせず、エラーを出す
    let dest = TempDir::new("dlg-over-no");
    let (mut h, fake) = harness(None);
    let ctx = h.ctx.clone();
    for _ in 0..2 {
        fake.folder
            .borrow_mut()
            .push_back(dest.path().to_path_buf());
    }
    app(&mut h).export_dialog(&ctx, false);
    fake.calls();
    app(&mut h).doc.template.card_color = 0x0012_3456;
    app(&mut h).export_dialog(&ctx, false);
    assert_eq!(modal_title(&h).as_deref(), Some("書き出せません"));
    assert!(modal_body(&h).unwrap().contains("同名"));
    let kept = Template::parse(
        &std::fs::read(dest.path().join("TMPEXP/TEMP1x3_V/新規テンプレート.TPL")).unwrap(),
    )
    .unwrap();
    assert_ne!(kept.card_color, 0x0012_3456, "拒否したら上書きしない");
}

// ----- ダイアログに依存しない部分 -----

#[test]
fn export_items_single_and_all() {
    let lib_dir = TempDir::new("dlg-items");
    let a = write_template(lib_dir.path(), "TEMP1x3_V", "甲");
    write_template(lib_dir.path(), "TEMP2x1_H", "乙");
    let lib = Library::scan(lib_dir.path()).unwrap();
    let doc = Document::open(&a).unwrap();

    let single = export_items(&doc, Some(&lib), false).unwrap();
    assert_eq!(single.len(), 1);
    assert_eq!(single[0].0, "甲");

    let all = export_items(&doc, Some(&lib), true).unwrap();
    let names: Vec<_> = all.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, vec!["甲", "乙"]);

    // 一覧が無ければ、まとめて指定しても今の 1 件だけ
    let none = export_items(&doc, None, true).unwrap();
    assert_eq!(none.len(), 1);
}

#[test]
fn export_items_uses_the_edited_template_for_the_open_file() {
    let lib_dir = TempDir::new("dlg-items-edit");
    let a = write_template(lib_dir.path(), "TEMP1x3_V", "甲");
    let lib = Library::scan(lib_dir.path()).unwrap();
    let mut doc = Document::open(&a).unwrap();
    doc.select_all();
    doc.delete_selected();
    let all = export_items(&doc, Some(&lib), true).unwrap();
    assert!(all[0].1.objects.is_empty());
    assert!(doc.template.objects.iter().all(|o| o.kind != KIND_CAPTION));
}

#[test]
fn export_with_confirm_only_asks_on_a_name_clash() {
    let dest = TempDir::new("dlg-confirm");
    let fake = FakeDialogs::default();
    let items = vec![("A".to_string(), doc::starter_template(Header::default()))];
    // 初回は衝突しないので確認しない
    let first = export_with_confirm(dest.path(), &items, &fake).unwrap();
    assert_eq!(first.len(), 1);
    assert!(fake.calls().is_empty());
    // 同名: 拒否 → エラー
    let e = export_with_confirm(dest.path(), &items, &fake).unwrap_err();
    assert!(e.contains("同名"), "{e}");
    assert_eq!(fake.calls().len(), 1);
    // 同名: 承諾 → 上書き
    fake.confirm.borrow_mut().push_back(true);
    assert!(export_with_confirm(dest.path(), &items, &fake).is_ok());
    // 同名以外のエラーでは確認しない
    let bad = vec![("a/b".to_string(), doc::starter_template(Header::default()))];
    let e = export_with_confirm(dest.path(), &bad, &fake).unwrap_err();
    assert!(e.contains("使用できない文字"), "{e}");
    assert_eq!(
        fake.calls().len(),
        1,
        "確認は同名のときの 1 回だけ（上の承諾分）"
    );
}

// ----- 設定の保存（最後に使ったフォルダ・表示の設定） -----

fn saves(store: &MemoryStore) -> Vec<Settings> {
    store.0.borrow().saves.clone()
}

fn custom_settings() -> Settings {
    Settings {
        folders: Folders {
            library: Some(PathBuf::from(r"C:\last\TMPEXP")),
            export_dest: Some(PathBuf::from(r"D:\out")),
        },
        view: ViewSettings {
            show_grid: false,
            grid_mm: 2.5,
            snap_grid: false,
            snap_objects: false,
            show_used: false,
            sample_values: true,
            theme: crate::theme::ThemeChoice::Dark,
        },
        colors: crate::settings::Colors {
            recent: vec![[0xff, 0x00, 0x00], [0x44, 0x72, 0xc4]],
        },
    }
}

#[test]
fn saved_settings_are_restored_at_startup_and_not_written_back() {
    let store = MemoryStore::default();
    store.0.borrow_mut().stored = custom_settings();
    let (mut h, _) = harness_with_store(None, store.clone());
    let a = app(&mut h);
    assert_eq!(ViewSettings::of(&a.view), custom_settings().view);
    assert_eq!(a.settings.folders, custom_settings().folders);
    assert!(a.modal.is_none() && a.toast.is_none());
    assert!(saves(&store).is_empty(), "読んだだけでは書かない");
}

#[test]
fn saved_theme_is_applied_to_the_context_at_startup() {
    use crate::theme::ThemeChoice;
    // テスト用のハーネスは、アプリを作ったあとに、自分のテーマを設定するので、素の Context に対して確かめる
    for choice in ThemeChoice::ALL {
        let store = MemoryStore::default();
        store.0.borrow_mut().stored.view.theme = choice;
        let ctx = egui::Context::default();
        let app = StudioApp::with_dialogs(
            &ctx,
            None,
            None,
            Box::new(Rc::new(FakeDialogs::default())),
            Box::new(store.clone()),
        );
        assert_eq!(app.view.theme, choice, "{choice:?}");
        assert_eq!(
            ctx.options(|o| o.theme_preference),
            choice.preference(),
            "{choice:?}"
        );
        assert!(saves(&store).is_empty(), "読んだだけでは書かない");
    }
    // 設定が無い起動は、システムに合わせる
    let ctx = egui::Context::default();
    let app = StudioApp::with_dialogs(
        &ctx,
        None,
        None,
        Box::new(Rc::new(FakeDialogs::default())),
        Box::new(MemoryStore::default()),
    );
    assert_eq!(app.view.theme, ThemeChoice::System);
    assert_eq!(
        ctx.options(|o| o.theme_preference),
        ThemeChoice::System.preference()
    );
}

#[test]
fn theme_chosen_from_the_view_menu_is_applied_and_saved() {
    use crate::theme::ThemeChoice;
    let store = MemoryStore::default();
    store.0.borrow_mut().stored = custom_settings(); // テーマは、ダーク
    let (mut h, _) = harness_with_store(None, store.clone());
    assert_eq!(app(&mut h).view.theme, ThemeChoice::Dark);
    // 「表示」メニューの 3 択で、ライトに変える → 画面に反映され、設定に書き出される
    open_menu(&mut h, "表示", ThemeChoice::Light.label());
    assert_eq!(app(&mut h).view.theme, ThemeChoice::Light);
    assert_eq!(h.ctx.theme(), egui::Theme::Light);
    let saved = saves(&store);
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].view.theme, ThemeChoice::Light);
    // ダークに戻す
    open_menu(&mut h, "表示", ThemeChoice::Dark.label());
    assert_eq!(h.ctx.theme(), egui::Theme::Dark);
    assert_eq!(saves(&store).last().unwrap().view.theme, ThemeChoice::Dark);
    // システムに合わせる → egui の設定が System になる
    open_menu(&mut h, "表示", ThemeChoice::System.label());
    assert_eq!(
        h.ctx.options(|o| o.theme_preference),
        ThemeChoice::System.preference()
    );
    assert_eq!(
        saves(&store).last().unwrap().view.theme,
        ThemeChoice::System
    );
}

#[test]
fn recent_colors_are_restored_at_startup_and_saved_when_they_change() {
    let store = MemoryStore::default();
    store.0.borrow_mut().stored = custom_settings();
    let (mut h, _) = harness_with_store(None, store.clone());
    // 起動時に、保存してあった最近使った色が、パレットに入る（読んだだけでは、書かない）
    assert_eq!(
        crate::palette::recent(&h.ctx),
        custom_settings().colors.recent
    );
    assert!(saves(&store).is_empty());
    // 色を選んで並びが変わると、設定に書き出される
    crate::palette::set_recent(&h.ctx, &[[0, 0xb0, 0x50], [0xff, 0x00, 0x00]]);
    h.run();
    let saved = saves(&store);
    assert_eq!(saved.len(), 1);
    assert_eq!(
        saved[0].colors.recent,
        vec![[0, 0xb0, 0x50], [0xff, 0x00, 0x00]]
    );
}

#[test]
fn unreadable_settings_start_with_defaults_and_tell_the_user() {
    let store = MemoryStore::default();
    store.0.borrow_mut().load_error = Some("settings.json の内容が不正です: 壊れている".into());
    let (mut h, _) = harness_with_store(None, store.clone());
    assert_eq!(modal_title(&h).as_deref(), Some("設定を読めませんでした"));
    let body = modal_body(&h).expect("本文");
    assert!(body.contains("壊れている"), "{body}");
    assert!(body.contains("既定の設定で起動しました"), "{body}");
    assert_eq!(ViewSettings::of(&app(&mut h).view), ViewSettings::default());
    assert!(saves(&store).is_empty());
}

#[test]
fn a_settings_error_does_not_hide_the_error_of_the_file_to_open() {
    // 起動時に開けなかったファイルのエラーを、設定のエラーで上書きしない（設定は通知に回す）
    let store = MemoryStore::default();
    store.0.borrow_mut().load_error = Some("読めない".into());
    let missing = TempDir::new("dlg-settings-open").path().join("無い.TPL");
    let (mut h, _) = harness_with_store(Some(missing), store);
    assert_eq!(modal_title(&h).as_deref(), Some("開けませんでした"));
    let toast = app(&mut h).toast.clone().expect("toast").0;
    assert!(toast.contains("設定を読めませんでした"), "{toast}");
}

#[test]
fn view_changes_are_saved_once_per_change_and_zoom_is_not_saved() {
    let store = MemoryStore::default();
    let (mut h, _) = harness_with_store(None, store.clone());
    app(&mut h).view.show_grid = false;
    h.run();
    h.run();
    assert_eq!(saves(&store).len(), 1, "変えたときだけ、1 回");
    assert!(!saves(&store)[0].view.show_grid);

    app(&mut h).view.grid_mm = 5.0;
    h.run();
    assert_eq!(saves(&store).len(), 2);
    assert_eq!(saves(&store)[1].view.grid_mm, 5.0);

    // 拡大率・スクロール位置は、カードに合わせて決まる。変わっても保存しない
    app(&mut h).view.zoom = 3.0;
    app(&mut h).view.pan = egui::vec2(10.0, 10.0);
    h.run();
    assert_eq!(saves(&store).len(), 2);
}

#[test]
fn a_failed_save_is_reported_once_and_retried_only_when_the_settings_change() {
    let store = MemoryStore::default();
    store.0.borrow_mut().save_error = Some("ディスクがいっぱいです".into());
    let (mut h, _) = harness_with_store(None, store.clone());

    app(&mut h).view.show_grid = false;
    h.run();
    let toast = app(&mut h).toast.take().expect("失敗を知らせる").0;
    assert!(toast.contains("設定を保存できませんでした"), "{toast}");
    assert!(toast.contains("ディスクがいっぱいです"), "{toast}");

    h.run();
    h.run();
    assert_eq!(saves(&store).len(), 1, "同じ内容を、毎フレーム試し直さない");
    assert!(app(&mut h).toast.is_none());

    // 失敗が続いている間は、変えるたびに試すが、知らせは繰り返さない
    app(&mut h).view.snap_grid = false;
    h.run();
    assert_eq!(saves(&store).len(), 2);
    assert!(app(&mut h).toast.is_none());

    // 直れば保存され、そのあとの失敗は、また知らせる
    store.0.borrow_mut().save_error = None;
    app(&mut h).view.snap_objects = false;
    h.run();
    assert_eq!(store.0.borrow().stored, saves(&store)[2]);
    assert!(app(&mut h).toast.is_none());

    store.0.borrow_mut().save_error = Some("再び書けません".into());
    app(&mut h).view.show_used = false;
    h.run();
    let toast = app(&mut h).toast.take().expect("また知らせる").0;
    assert!(toast.contains("再び書けません"), "{toast}");
}

#[test]
fn opening_a_library_remembers_the_folder_for_the_next_dialog() {
    let tmp = TempDir::new("dlg-remember-lib");
    write_template(tmp.path(), "TEMP1x3_V", "a");
    let store = MemoryStore::default();
    let (mut h, fake) = harness_with_store(None, store.clone());
    let menu = "エクスポートしたフォルダ（TMPEXP）を開く";
    let title = "TMPEXP フォルダを選んでください";

    fake.folder.borrow_mut().push_back(tmp.path().to_path_buf());
    open_menu(&mut h, "ファイル", menu);
    assert_eq!(
        fake.calls(),
        vec![Call::Folder(Some(title.into()), None)],
        "初回は開始位置の指定なし"
    );
    let last = saves(&store).pop().expect("保存された");
    assert_eq!(last.folders.library.as_deref(), Some(tmp.path()));

    fake.folder.borrow_mut().push_back(tmp.path().to_path_buf());
    open_menu(&mut h, "ファイル", menu);
    assert_eq!(
        fake.calls(),
        vec![Call::Folder(
            Some(title.into()),
            Some(tmp.path().to_path_buf())
        )],
        "2 回目は、前に開いたフォルダから始める"
    );
}

#[test]
fn a_folder_without_templates_is_not_remembered() {
    let tmp = TempDir::new("dlg-remember-empty");
    let store = MemoryStore::default();
    let (mut h, fake) = harness_with_store(None, store.clone());
    fake.folder.borrow_mut().push_back(tmp.path().to_path_buf());
    h.get_by_label("📂 TMPEXP フォルダを開く…").click();
    h.run();
    assert!(modal_title(&h).is_some());
    assert!(saves(&store).is_empty());
}

#[test]
fn the_open_dialog_starts_in_the_last_library_when_none_is_open() {
    let store = MemoryStore::default();
    store.0.borrow_mut().stored = custom_settings();
    let (mut h, fake) = harness_with_store(None, store);
    app(&mut h).open_dialog();
    assert_eq!(
        fake.calls(),
        vec![Call::Open(Some(PathBuf::from(r"C:\last\TMPEXP")))]
    );
}

#[test]
fn a_successful_export_remembers_the_destination() {
    let dest = TempDir::new("dlg-remember-dest");
    let store = MemoryStore::default();
    let (mut h, fake) = harness_with_store(None, store.clone());
    let ctx = h.ctx.clone();

    fake.folder
        .borrow_mut()
        .push_back(dest.path().to_path_buf());
    app(&mut h).export_dialog(&ctx, false);
    h.run();
    let last = saves(&store).pop().expect("保存された");
    assert_eq!(last.folders.export_dest.as_deref(), Some(dest.path()));

    fake.calls();
    app(&mut h).export_dialog(&ctx, false);
    assert_eq!(
        fake.calls(),
        vec![Call::Folder(
            Some(EXPORT_TITLE.into()),
            Some(dest.path().to_path_buf())
        )],
        "次は、前の書き出し先から始める"
    );
}

#[test]
fn settings_survive_a_restart_through_a_real_file() {
    let dir = TempDir::new("dlg-settings-file");
    let path = dir.path().join("settings.json");
    {
        let (mut h, _) = harness_with_store(None, FileStore::at(path.clone()));
        let a = app(&mut h);
        a.view.show_grid = false;
        a.view.grid_mm = 3.0;
        h.run();
    }
    let json = std::fs::read_to_string(&path).expect("保存された");
    assert!(json.contains("\"grid_mm\": 3.0"), "{json}");

    let (mut h, _) = harness_with_store(None, FileStore::at(path));
    let a = app(&mut h);
    assert!(!a.view.show_grid);
    assert_eq!(a.view.grid_mm, 3.0);
    assert!(a.modal.is_none());
}

#[test]
fn a_corrupt_settings_file_is_reported_and_replaced_by_the_next_change() {
    let dir = TempDir::new("dlg-settings-corrupt");
    let path = dir.path().join("settings.json");
    std::fs::write(&path, "{ 壊れた").unwrap();
    let (mut h, _) = harness_with_store(None, FileStore::at(path.clone()));
    assert_eq!(modal_title(&h).as_deref(), Some("設定を読めませんでした"));
    assert!(modal_body(&h).unwrap().contains("settings.json"));
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "{ 壊れた",
        "起動しただけでは、ファイルに触れない"
    );

    app(&mut h).view.show_grid = false;
    h.run();
    let after = std::fs::read_to_string(&path).unwrap();
    assert!(after.contains("\"show_grid\": false"), "{after}");
}
