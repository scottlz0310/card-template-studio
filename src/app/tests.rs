use super::*;
use crate::settings::memory::MemoryStore;
use crate::test_support::TempDir;
use crate::tpl::{KIND_CAPTION, KIND_PICTURE};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

type H = Harness<'static, StudioApp>;

fn harness_with(open: Option<PathBuf>, screenshot: Option<PathBuf>) -> H {
    let mut h = Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .allow_missing_glyphs()
        .with_accessibility_check(true)
        .with_max_steps(30)
        .build_eframe(move |cc| {
            StudioApp::new(
                &cc.egui_ctx,
                open,
                screenshot,
                Box::new(MemoryStore::default()),
            )
        });
    if h.state().shot.is_some() {
        // 画面を撮るモードは常に再描画を求めるので、落ち着くのを待たない
        h.step();
    } else {
        h.run();
    }
    h
}

fn harness() -> H {
    harness_with(None, None)
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

fn is_confirm(h: &H) -> bool {
    matches!(h.state().modal, Some(Modal::ConfirmDiscard(_)))
}

fn write_template(dir: &std::path::Path, folder: &str, name: &str) -> PathBuf {
    let path = dir.join(folder).join(format!("{name}.TPL"));
    let mut d = Document::new(doc::starter_template(Header::default()), name, None);
    d.save_to(&path).expect("save sample");
    path
}

// ----- 起動 -----

#[test]
fn starts_with_starter_template() {
    let mut h = harness();
    let a = app(&mut h);
    assert_eq!(a.doc.name, "新規テンプレート");
    assert_eq!(a.doc.template.objects.len(), 11);
    assert!(!a.doc.dirty && a.doc.path.is_none());
    assert!(a.modal.is_none() && a.library.is_none() && a.shot.is_none());
}

#[test]
fn startup_argument_opens_file_or_library() {
    let tmp = TempDir::new("app-open");
    let file = write_template(tmp.path(), "TEMP1x3_V", "見本");

    let mut h = harness_with(Some(file.clone()), None);
    assert_eq!(app(&mut h).doc.name, "見本");
    assert_eq!(app(&mut h).doc.path.as_deref(), Some(file.as_path()));

    let mut h = harness_with(Some(tmp.path().to_path_buf()), None);
    let lib = app(&mut h).library.clone().expect("library");
    assert_eq!(lib.entries.len(), 1);
    assert_eq!(lib.entries[0].name, "見本");

    let h = harness_with(Some(tmp.path().join("無い.TPL")), None);
    assert_eq!(modal_title(&h).as_deref(), Some("開けませんでした"));
}

#[test]
fn screenshot_mode_prepares_a_selected_sample() {
    let tmp = TempDir::new("app-shot");
    let path = tmp.path().join("out").join("shot.png");
    let mut h = harness_with(None, Some(path.clone()));
    let a = app(&mut h);
    assert!(a.shot.is_some());
    assert_eq!(a.doc.name, "見本テンプレート");
    let first_caption = a
        .doc
        .template
        .objects
        .iter()
        .position(|o| o.kind == KIND_CAPTION)
        .unwrap();
    assert_eq!(a.doc.selection, [first_caption + 1].into());
}

#[test]
fn screenshot_is_written_and_app_closes() {
    let tmp = TempDir::new("app-shot-write");
    let path = tmp.path().join("out").join("shot.png");
    let mut h = harness_with(None, Some(path.clone()));
    let image = Arc::new(egui::ColorImage::filled(
        [4, 3],
        Color32::from_rgb(10, 20, 30),
    ));
    {
        let shot = app(&mut h).shot.as_mut().unwrap();
        *shot.image.lock().unwrap() = Some(image);
    }
    h.step();
    assert!(path.exists(), "画面を PNG に保存する");
    let png = image::open(&path).unwrap().to_rgba8();
    assert_eq!(png.dimensions(), (4, 3));
    assert_eq!(png.get_pixel(0, 0).0, [10, 20, 30, 255]);
    assert!(app(&mut h).allow_close);
}

#[test]
fn screenshot_is_requested_after_warmup_frames() {
    let tmp = TempDir::new("app-shot-req");
    let mut h = harness_with(None, Some(tmp.path().join("s.png")));
    for _ in 0..130 {
        h.step();
    }
    assert!(app(&mut h).shot.as_ref().unwrap().requested);
}

// ----- ファイル操作 -----

#[test]
fn open_library_reports_empty_and_missing_folders() {
    let tmp = TempDir::new("app-lib-err");
    let mut h = harness();
    app(&mut h).open_library(tmp.path().to_path_buf());
    assert_eq!(
        modal_title(&h).as_deref(),
        Some("テンプレートが見つかりません")
    );
    app(&mut h).modal = None;
    app(&mut h).open_library(tmp.path().join("無い"));
    assert_eq!(modal_title(&h).as_deref(), Some("フォルダを開けません"));
    assert!(app(&mut h).library.is_none());
}

#[test]
fn open_file_resets_view_and_tool() {
    let tmp = TempDir::new("app-openfile");
    let file = write_template(tmp.path(), "TEMP1x3_V", "a");
    let mut h = harness();
    app(&mut h).tool = Tool::Row;
    app(&mut h).view.fit_pending = false;
    app(&mut h).open_file(file);
    assert_eq!(app(&mut h).tool, Tool::Select);
    assert!(app(&mut h).view.fit_pending);
}

#[test]
fn save_writes_to_known_path_and_toasts() {
    let tmp = TempDir::new("app-save");
    let path = tmp.path().join("保存.TPL");
    let mut h = harness();
    app(&mut h).doc.checkpoint(None);
    let ctx = h.ctx.clone();
    app(&mut h).save_to(path.clone(), &ctx);
    assert!(path.exists());
    assert!(!app(&mut h).doc.dirty);
    let toast = app(&mut h).toast.clone().expect("toast");
    assert!(toast.0.starts_with("保存しました"), "{}", toast.0);
    // 以降の「保存」は同じ場所へ
    app(&mut h).doc.checkpoint(None);
    app(&mut h).doc.template.card_color = 0x0012_3456;
    app(&mut h).save(&ctx);
    let back = Document::open(&path).unwrap();
    assert_eq!(back.template.card_color, 0x0012_3456);
}

#[test]
fn save_without_destination_asks_for_a_name() {
    let mut h = harness();
    let ctx = h.ctx.clone();
    app(&mut h).save(&ctx);
    assert!(matches!(
        app(&mut h).modal,
        Some(Modal::SaveAsName { ref name, .. }) if name == "新規テンプレート"
    ));
}

#[test]
fn save_inside_library_uses_layout_folder_and_rescans() {
    let tmp = TempDir::new("app-save-lib");
    write_template(tmp.path(), "TEMP1x3_V", "既存");
    let mut h = harness_with(Some(tmp.path().to_path_buf()), None);
    let ctx = h.ctx.clone();
    app(&mut h).doc.name = "新作".into();
    app(&mut h).doc.checkpoint(None);
    app(&mut h).save(&ctx);
    assert!(tmp.path().join("TEMP1x3_V").join("新作.TPL").exists());
    let names: Vec<_> = app(&mut h)
        .library
        .as_ref()
        .unwrap()
        .entries
        .iter()
        .map(|e| e.name.clone())
        .collect();
    assert_eq!(names, vec!["新作", "既存"], "名前の文字コード順");
}

#[test]
fn save_rejects_invalid_names_without_writing() {
    let tmp = TempDir::new("app-save-bad");
    let mut h = harness();
    let ctx = h.ctx.clone();
    app(&mut h).save_to(tmp.path().join("a|b.TPL"), &ctx);
    assert_eq!(modal_title(&h).as_deref(), Some("保存できません"));
    assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
}

#[test]
fn save_failure_is_reported() {
    let tmp = TempDir::new("app-save-fail");
    let blocker = tmp.path().join("file");
    std::fs::write(&blocker, b"x").unwrap();
    let mut h = harness();
    let ctx = h.ctx.clone();
    app(&mut h).doc.checkpoint(None);
    app(&mut h).save_to(blocker.join("x.TPL"), &ctx);
    assert_eq!(modal_title(&h).as_deref(), Some("保存できません"));
    assert!(app(&mut h).doc.dirty);
}

#[test]
fn guard_asks_before_discarding_changes() {
    let tmp = TempDir::new("app-guard");
    let file = write_template(tmp.path(), "TEMP1x3_V", "a");
    let mut h = harness();
    // 変更なし: そのまま実行
    app(&mut h).guard(Pending::Open(file.clone()));
    assert_eq!(app(&mut h).doc.name, "a");
    app(&mut h).guard(Pending::Close);
    assert!(app(&mut h).allow_close);
    app(&mut h).allow_close = false;
    app(&mut h).guard(Pending::New);
    assert!(matches!(
        app(&mut h).modal,
        Some(Modal::NewTemplate {
            layout: (1, 3, true),
            ..
        })
    ));
    // 変更あり: 確認の画面
    app(&mut h).modal = None;
    app(&mut h).doc.checkpoint(None);
    app(&mut h).guard(Pending::Open(file));
    assert!(is_confirm(&h));
    assert!(!app(&mut h).allow_close);
}

// ----- 画面（確認の画面）-----

#[test]
fn new_template_dialog_creates_starter_or_blank() {
    let cases = [("写真枠と「工種", 11usize), ("白紙から始める", 0)];
    for (radio, objects) in cases {
        let mut h = harness();
        app(&mut h).run_pending(Pending::New);
        h.run();
        h.get_by_label_contains(radio).click();
        h.run();
        h.get_by_label("作成").click();
        h.run();
        let a = app(&mut h);
        assert!(a.modal.is_none(), "{radio}");
        assert_eq!(a.doc.template.objects.len(), objects, "{radio}");
        assert_eq!(a.doc.name, "新規テンプレート");
        assert!(a.doc.dirty, "作ったばかりは未保存");
        assert!(a.doc.path.is_none());
    }
}

#[test]
fn new_template_dialog_rejects_bad_name_and_can_be_cancelled() {
    let mut h = harness();
    app(&mut h).modal = Some(Modal::NewTemplate {
        name: "a/b".into(),
        layout: (2, 2, false),
        starter: true,
        error: None,
    });
    h.run();
    h.get_by_label("作成").click();
    h.run();
    assert!(matches!(
        app(&mut h).modal,
        Some(Modal::NewTemplate { error: Some(_), .. })
    ));
    h.get_by_label_contains("使用できない文字");
    h.get_by_label("やめる").click();
    h.run();
    assert!(app(&mut h).modal.is_none());
    assert_eq!(app(&mut h).doc.name, "新規テンプレート");
}

#[test]
fn new_template_uses_chosen_layout() {
    let mut h = harness();
    app(&mut h).modal = Some(Modal::NewTemplate {
        name: " 横長 ".into(),
        layout: (2, 1, false),
        starter: false,
        error: None,
    });
    h.run();
    h.get_by_label("作成").click();
    h.run();
    let a = app(&mut h);
    assert_eq!(a.doc.name, "横長");
    let hd = &a.doc.template.header;
    assert_eq!((hd.cols, hd.rows, hd.is_portrait()), (2, 1, false));
}

#[test]
fn confirm_dialog_choices() {
    let tmp = TempDir::new("app-confirm");
    let path = tmp.path().join("確認.TPL");
    // 保存しない → 変更を捨てて、保留していた操作（新規作成）へ進む
    let mut h = harness();
    app(&mut h).doc.checkpoint(None);
    app(&mut h).guard(Pending::New);
    h.run();
    h.get_by_label("保存しない").click();
    h.run();
    assert!(!app(&mut h).doc.dirty);
    assert!(matches!(app(&mut h).modal, Some(Modal::NewTemplate { .. })));
    // やめる → 何も起きない
    let mut h = harness();
    app(&mut h).doc.checkpoint(None);
    app(&mut h).guard(Pending::New);
    h.run();
    h.get_by_label("やめる").click();
    h.run();
    assert!(app(&mut h).doc.dirty && app(&mut h).modal.is_none());
    // 保存する → 保存してから保留していた操作へ
    let mut h = harness();
    app(&mut h).doc.path = Some(path.clone());
    app(&mut h).doc.checkpoint(None);
    app(&mut h).guard(Pending::New);
    h.run();
    h.get_by_label("保存する").click();
    h.run();
    assert!(path.exists());
    assert!(matches!(app(&mut h).modal, Some(Modal::NewTemplate { .. })));
    // 保存先が決まらないときは名前の画面へ進み、保留していた操作は行わない
    let mut h = harness();
    app(&mut h).doc.checkpoint(None);
    app(&mut h).guard(Pending::New);
    h.run();
    h.get_by_label("保存する").click();
    h.run();
    assert!(matches!(app(&mut h).modal, Some(Modal::SaveAsName { .. })));
    // 閉じる操作の保留は、保存しないを選ぶと終了の許可になる
    let mut h = harness();
    app(&mut h).doc.checkpoint(None);
    app(&mut h).guard(Pending::Close);
    h.run();
    h.get_by_label("保存しない").click();
    h.step();
    assert!(app(&mut h).allow_close && !app(&mut h).doc.dirty);
}

#[test]
fn save_as_name_dialog_validates_and_cancels() {
    let mut h = harness();
    app(&mut h).modal = Some(Modal::SaveAsName {
        name: "  ".into(),
        error: None,
    });
    h.run();
    h.get_by_label("保存先を選ぶ…").click();
    h.run();
    assert!(matches!(
        app(&mut h).modal,
        Some(Modal::SaveAsName { error: Some(_), .. })
    ));
    h.get_by_label("やめる").click();
    h.run();
    assert!(app(&mut h).modal.is_none());
}

#[test]
fn message_dialog_closes() {
    let mut h = harness();
    app(&mut h).error("題名", "本文");
    h.run();
    h.get_by_label("題名");
    h.get_by_label("本文");
    h.get_by_label("閉じる").click();
    h.run();
    assert!(app(&mut h).modal.is_none());
}

#[test]
fn modal_blocks_shortcuts() {
    let mut h = harness();
    app(&mut h).error("t", "b");
    h.run();
    press(&mut h, Modifiers::COMMAND, Key::A);
    assert!(app(&mut h).doc.selection.is_empty());
}

#[test]
fn close_request_with_unsaved_changes_asks_first() {
    let mut h = harness();
    app(&mut h).doc.checkpoint(None);
    let id = h.ctx.viewport_id();
    h.input_mut()
        .viewports
        .entry(id)
        .or_default()
        .events
        .push(egui::ViewportEvent::Close);
    h.step();
    h.input_mut().viewports.clear();
    assert!(is_confirm(&h));
    // 変更が無ければ確認なし
    let mut h = harness();
    let id = h.ctx.viewport_id();
    h.input_mut()
        .viewports
        .entry(id)
        .or_default()
        .events
        .push(egui::ViewportEvent::Close);
    h.step();
    assert!(app(&mut h).modal.is_none());
}

#[test]
fn toast_disappears_after_a_while() {
    let mut h = harness();
    app(&mut h).toast = Some(("保存しました".into(), 0.0));
    h.step();
    assert!(app(&mut h).toast.is_some() || h.ctx.input(|i| i.time) > 6.0);
    app(&mut h).toast = Some(("古い".into(), -100.0));
    h.step();
    assert!(app(&mut h).toast.is_none());
}

#[test]
fn window_title_follows_document_state() {
    let mut h = harness();
    assert_eq!(
        app(&mut h).last_title,
        "新規テンプレート - カードテンプレート工房"
    );
    app(&mut h).doc.checkpoint(None);
    h.step();
    assert_eq!(
        app(&mut h).last_title,
        "新規テンプレート * - カードテンプレート工房"
    );
}

// ----- ショートカット -----

fn selected_count(h: &mut H) -> usize {
    app(h).doc.selection.len()
}

#[test]
fn select_all_copy_paste_cut_duplicate_delete() {
    let mut h = harness();
    press(&mut h, Modifiers::COMMAND, Key::A);
    assert_eq!(selected_count(&mut h), 11);
    press(&mut h, Modifiers::COMMAND, Key::C);
    assert_eq!(app(&mut h).clipboard.len(), 11);
    press(&mut h, Modifiers::COMMAND, Key::V);
    assert_eq!(app(&mut h).doc.template.objects.len(), 22);
    assert_eq!(selected_count(&mut h), 11);
    press(&mut h, Modifiers::COMMAND, Key::D);
    assert_eq!(app(&mut h).doc.template.objects.len(), 33);
    press(&mut h, Modifiers::NONE, Key::Delete);
    assert_eq!(app(&mut h).doc.template.objects.len(), 22);
    press(&mut h, Modifiers::COMMAND, Key::A);
    press(&mut h, Modifiers::COMMAND, Key::X);
    assert!(app(&mut h).doc.template.objects.is_empty());
    assert_eq!(app(&mut h).clipboard.len(), 22);
}

#[test]
fn undo_and_redo_shortcuts() {
    let mut h = harness();
    press(&mut h, Modifiers::COMMAND, Key::A);
    press(&mut h, Modifiers::NONE, Key::Backspace);
    assert!(app(&mut h).doc.template.objects.is_empty());
    press(&mut h, Modifiers::COMMAND, Key::Z);
    assert_eq!(app(&mut h).doc.template.objects.len(), 11);
    press(&mut h, Modifiers::COMMAND, Key::Y);
    assert!(app(&mut h).doc.template.objects.is_empty());
    press(&mut h, Modifiers::COMMAND, Key::Z);
    assert_eq!(app(&mut h).doc.template.objects.len(), 11);
    // Ctrl+Shift+Z もやり直し（Ctrl+Z に先取りされない）
    press(&mut h, Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    assert!(app(&mut h).doc.template.objects.is_empty());
    assert!(!app(&mut h).doc.can_redo());
    press(&mut h, Modifiers::COMMAND, Key::Z);
    assert_eq!(app(&mut h).doc.template.objects.len(), 11);
    assert!(app(&mut h).doc.can_redo());
}

#[test]
fn tool_keys_and_escape() {
    let cases = [
        (Key::P, Tool::Picture),
        (Key::T, Tool::Label),
        (Key::D, Tool::Value),
        (Key::R, Tool::Row),
        (Key::V, Tool::Select),
    ];
    let mut h = harness();
    for (key, tool) in cases {
        press(&mut h, Modifiers::NONE, key);
        assert_eq!(app(&mut h).tool, tool, "{key:?}");
    }
    press(&mut h, Modifiers::NONE, Key::R);
    press(&mut h, Modifiers::COMMAND, Key::A);
    press(&mut h, Modifiers::NONE, Key::Escape);
    assert_eq!(app(&mut h).tool, Tool::Select);
    assert!(app(&mut h).doc.selection.is_empty());
}

#[test]
fn arrow_keys_nudge_selection() {
    let mut h = harness();
    let first = app(&mut h).doc.template.objects[0].rect;
    app(&mut h).doc.selection = [0].into();
    press(&mut h, Modifiers::NONE, Key::ArrowRight);
    let r = app(&mut h).doc.template.objects[0].rect;
    assert!((r[0] - first[0] - 0.5 * MM).abs() < 1e-9);
    press(&mut h, Modifiers::SHIFT, Key::ArrowDown);
    let r2 = app(&mut h).doc.template.objects[0].rect;
    assert!((r2[1] - first[1] - 5.0 * MM).abs() < 1e-9);
    press(&mut h, Modifiers::NONE, Key::ArrowLeft);
    press(&mut h, Modifiers::NONE, Key::ArrowUp);
    let r3 = app(&mut h).doc.template.objects[0].rect;
    assert!((r3[0] - first[0]).abs() < 1e-9);
}

#[test]
fn fit_shortcut_requests_refit() {
    let mut h = harness();
    app(&mut h).view.zoom = 3.0;
    press(&mut h, Modifiers::COMMAND, Key::Num0);
    assert!(app(&mut h).view.zoom < 3.0, "全体表示に戻る");
}

#[test]
fn new_shortcut_opens_new_template_dialog() {
    let mut h = harness();
    press(&mut h, Modifiers::COMMAND, Key::N);
    assert!(matches!(app(&mut h).modal, Some(Modal::NewTemplate { .. })));
    let mut h = harness();
    app(&mut h).doc.checkpoint(None);
    press(&mut h, Modifiers::COMMAND, Key::N);
    assert!(is_confirm(&h));
}

#[test]
fn save_shortcut_saves_to_known_path() {
    let tmp = TempDir::new("app-ctrl-s");
    let path = tmp.path().join("s.TPL");
    let mut h = harness();
    app(&mut h).doc.path = Some(path.clone());
    press(&mut h, Modifiers::COMMAND, Key::S);
    assert!(path.exists());
}

// ----- メニュー・道具 -----

fn open_menu(h: &mut H, menu: &str, item: &str) {
    // 同名の見出しが右の欄にもあるので、画面の上にあるメニューバー側を取る
    h.get_all_by_label(menu).next().expect("メニュー").click();
    h.run();
    // 配置のボタンは右の欄にも同じ名前がある。どちらも同じ操作をするので、後ろの方を押す
    h.get_all_by_label_contains(item)
        .last()
        .expect("メニュー項目")
        .click();
    h.run();
}

#[test]
fn part_list_shows_rows_and_selects_by_click_target() {
    // 見本の部品: 0 = 写真枠、以降は (項目名, 項目値) の組が 1〜2、3〜4、5〜6、… の順。一覧は位置順で、組は 1 行にまとまる
    // クリック先は、一覧の文字（項目値は `〈…〉`、項目名はその文字）か、`行N`（行番号。行全体）
    // (名前, クリック列 (クリック先, 修飾), 最後の選択)
    type Case = (&'static str, Vec<(&'static str, Modifiers)>, Vec<usize>);
    let none = Modifiers::NONE;
    let cases: Vec<Case> = vec![
        (
            "値の側のクリックは、値だけ",
            vec![("〈工種〉", none)],
            vec![2],
        ),
        (
            "項目名の側のクリックは、項目名だけ",
            vec![("工種", none)],
            vec![1],
        ),
        (
            "行番号のクリックは、行全体（項目名と値）",
            vec![("行2", none)],
            vec![3, 4],
        ),
        (
            "Shift+クリックは、起点の行から押した行までを、行単位で選ぶ",
            vec![("〈工種〉", none), ("〈写真タイトル〉", Modifiers::SHIFT)],
            vec![1, 2, 3, 4, 5, 6],
        ),
        (
            "行番号から行番号への Shift+クリックも、行単位",
            vec![("行1", none), ("行3", Modifiers::SHIFT)],
            vec![1, 2, 3, 4, 5, 6],
        ),
        (
            "Ctrl+クリックは、その部品だけを足す（範囲にならない）",
            vec![("〈工種〉", none), ("〈写真タイトル〉", Modifiers::COMMAND)],
            vec![2, 6],
        ),
        (
            "Ctrl+行番号は、行全体を選択に足す",
            vec![("〈工種〉", none), ("行3", Modifiers::COMMAND)],
            vec![2, 5, 6],
        ),
        (
            "行全体が選ばれた行の Ctrl+行番号は、その行を外す",
            vec![("行3", none), ("行3", Modifiers::COMMAND)],
            vec![],
        ),
        (
            "Ctrl+クリックのあとの Shift+クリックは、Ctrl で押した部品の行が起点",
            vec![
                ("〈工種〉", none),
                ("〈撮影年月日〉", Modifiers::COMMAND),
                ("〈撮影箇所〉", Modifiers::SHIFT),
            ],
            vec![7, 8, 9, 10],
        ),
    ];
    for (name, clicks, want) in cases {
        let mut h = harness();
        app(&mut h).doc.selection.clear();
        for (target, mods) in clicks {
            if let Some(n) = target.strip_prefix("行") {
                h.get_by_label(&format!("{n} 行目の全体"))
                    .click_modifiers(mods);
            } else {
                h.get_by_label(target).click_modifiers(mods);
            }
            h.run();
        }
        let got: Vec<usize> = app(&mut h).doc.selection.iter().copied().collect();
        assert_eq!(got, want, "{name}");
    }
}

#[test]
fn part_list_is_in_position_order_with_rows_grouped() {
    let mut h = harness();
    let entries = app(&mut h).doc.layer_entries();
    // 写真枠が先頭、続いて 5 つの行（項目名・項目値）が上から並ぶ
    assert_eq!(entries[0], crate::doc::LayerEntry::Part(0));
    let rows: Vec<Vec<usize>> = entries[1..].iter().map(|e| e.members()).collect();
    assert_eq!(
        rows,
        [[1, 2], [3, 4], [5, 6], [7, 8], [9, 10]].map(|r| r.to_vec())
    );
}

#[test]
fn ctrl_enter_and_the_edit_menu_add_a_row_below() {
    let mut h = harness();
    let before = app(&mut h).doc.template.objects.len();
    // 何も選んでいないときは何も起きない
    app(&mut h).doc.selection.clear();
    press(&mut h, Modifiers::COMMAND, Key::Enter);
    assert_eq!(app(&mut h).doc.template.objects.len(), before);
    // 見本の最初の行（項目名 = #2）を 1 つ選ぶと、その行ぜんぶ（項目名と項目値）の下に足される
    app(&mut h).doc.selection = [1].into();
    press(&mut h, Modifiers::COMMAND, Key::Enter);
    assert_eq!(app(&mut h).doc.template.objects.len(), before + 2);
    assert_eq!(selected_count(&mut h), 2);
    // メニューからも同じ。1 回で元に戻る
    open_menu(&mut h, "編集", "下に行を追加");
    assert_eq!(app(&mut h).doc.template.objects.len(), before + 4);
    press(&mut h, Modifiers::COMMAND, Key::Z);
    assert_eq!(app(&mut h).doc.template.objects.len(), before + 2);
}

#[test]
fn add_row_below_that_would_leave_the_card_is_refused_with_a_toast() {
    let mut h = harness();
    // 表（文字項目）の最下段の下端を、カードの下端に合わせる（もう 1 行は入らない）
    {
        let doc = &mut app(&mut h).doc;
        let card_bottom = crate::layout::a4_division(&doc.template.header).1;
        let (last, lowest) = doc
            .template
            .objects
            .iter()
            .enumerate()
            .filter(|(_, o)| o.kind == crate::tpl::KIND_CAPTION)
            .map(|(i, o)| (i, o.normalized()[3]))
            .fold((0, 0.0), |m, c| if c.1 > m.1 { c } else { m });
        for o in &mut doc.template.objects {
            o.rect[1] += card_bottom - lowest;
            o.rect[3] += card_bottom - lowest;
        }
        doc.selection = [last].into();
    }
    let before = app(&mut h).doc.template.objects.len();
    press(&mut h, Modifiers::COMMAND, Key::Enter);
    assert_eq!(app(&mut h).doc.template.objects.len(), before);
    let toast = app(&mut h)
        .toast
        .clone()
        .map(|(m, _)| m)
        .unwrap_or_default();
    assert!(toast.contains("カードの下端に収まらない"), "{toast}");
    assert!(app(&mut h).doc.notice.is_none());
}

#[test]
fn edit_menu_commands() {
    let mut h = harness();
    open_menu(&mut h, "編集", "すべて選択");
    assert_eq!(selected_count(&mut h), 11);
    open_menu(&mut h, "編集", "コピー");
    assert_eq!(app(&mut h).clipboard.len(), 11);
    open_menu(&mut h, "編集", "貼り付け");
    assert_eq!(app(&mut h).doc.template.objects.len(), 22);
    open_menu(&mut h, "編集", "複製");
    assert_eq!(app(&mut h).doc.template.objects.len(), 33);
    open_menu(&mut h, "編集", "削除");
    assert_eq!(app(&mut h).doc.template.objects.len(), 22);
}

#[test]
fn view_menu_toggles() {
    let mut h = harness();
    assert!(app(&mut h).view.show_grid);
    open_menu(&mut h, "表示", "方眼を表示");
    assert!(!app(&mut h).view.show_grid);
    open_menu(&mut h, "表示", "方眼に吸着");
    assert!(!app(&mut h).view.snap_grid);
    open_menu(&mut h, "表示", "ほかの部品・カードの端に吸着");
    assert!(!app(&mut h).view.snap_objects);
    open_menu(&mut h, "表示", "既存テンプレートの配置範囲");
    assert!(!app(&mut h).view.show_used);
    open_menu(&mut h, "表示", "項目値に見本の文字を出す");
    assert!(app(&mut h).view.sample_values);
    app(&mut h).view.zoom = 3.0;
    open_menu(&mut h, "表示", "カード全体を表示");
    assert!(app(&mut h).view.zoom < 3.0);
}

#[test]
fn help_menu_shows_usage() {
    let mut h = harness();
    open_menu(&mut h, "ヘルプ", "使い方");
    assert_eq!(modal_title(&h).as_deref(), Some("使い方"));
    h.get_by_label_contains("本体との受け渡し");
}

#[test]
fn align_menu_works_on_multi_selection() {
    let mut h = harness();
    let ids: Vec<usize> = h
        .state()
        .doc
        .template
        .objects
        .iter()
        .enumerate()
        .filter(|(_, o)| o.kind == KIND_CAPTION)
        .map(|(i, _)| i)
        .take(3)
        .collect();
    app(&mut h).doc.selection = ids.iter().copied().collect();
    h.run();
    open_menu(&mut h, "配置", "幅を揃える");
    let w: Vec<f64> = ids
        .iter()
        .map(|&i| {
            let n = h.state().doc.template.objects[i].normalized();
            n[2] - n[0]
        })
        .collect();
    assert!(w.iter().all(|x| (x - w[0]).abs() < 1e-9), "{w:?}");
}

#[test]
fn toolbar_tool_buttons_and_zoom() {
    let mut h = harness();
    h.get_by_label("▭ 項目値").click();
    h.run();
    assert_eq!(app(&mut h).tool, Tool::Value);
    h.get_by_label("🖼 写真枠").click();
    h.run();
    assert_eq!(app(&mut h).tool, Tool::Picture);
    let z = app(&mut h).view.zoom;
    h.get_by_label("＋").click();
    h.run();
    assert!((app(&mut h).view.zoom - z * 1.25).abs() < 1e-3);
    h.get_by_label("－").click();
    h.get_by_label("－").click();
    h.run();
    assert!(app(&mut h).view.zoom < z);
    h.get_by_label("全体").click();
    h.run();
    h.get_by_label("方眼吸着").click();
    h.run();
    assert!(!app(&mut h).view.snap_grid);
    h.get_by_label("ガイド吸着").click();
    h.run();
    assert!(!app(&mut h).view.snap_objects);
    h.get_by_label("見本表示").click();
    h.run();
    assert!(app(&mut h).view.sample_values);
}

#[test]
fn toolbar_undo_redo_and_save() {
    let tmp = TempDir::new("app-toolbar-save");
    let path = tmp.path().join("t.TPL");
    let mut h = harness();
    app(&mut h).doc.path = Some(path.clone());
    app(&mut h).doc.select_all();
    app(&mut h).doc.delete_selected();
    h.run();
    h.get_by_label("↶ 元に戻す").click();
    h.run();
    assert_eq!(app(&mut h).doc.template.objects.len(), 11);
    h.get_by_label("↷ やり直し").click();
    h.run();
    assert!(app(&mut h).doc.template.objects.is_empty());
    h.get_by_label("💾 保存").click();
    h.run();
    assert!(path.exists());
}

// ----- 左右の欄 -----

#[test]
fn layer_list_selects_with_modifiers() {
    let mut h = harness();
    let mut labels = h.get_all_by_label_contains("〈").collect::<Vec<_>>();
    assert_eq!(labels.len(), 5, "項目値の行");
    labels.remove(0).click();
    h.run();
    assert_eq!(selected_count(&mut h), 1);
    // Shift+クリックは範囲選択: 起点（先頭の行の項目値）から 2 つ目の行の項目値まで、行単位で 4 つ（2 行ぶん）
    let second = h.get_all_by_label_contains("〈").nth(1).unwrap();
    second.click_modifiers(Modifiers::SHIFT);
    h.run();
    assert_eq!(selected_count(&mut h), 4);
    // 同じ部品を Shift+クリックし直しても、起点は動かず、範囲は変わらない
    let second = h.get_all_by_label_contains("〈").nth(1).unwrap();
    second.click_modifiers(Modifiers::SHIFT);
    h.run();
    assert_eq!(selected_count(&mut h), 4);
    // 個別に外す・戻すのは Ctrl+クリック
    let second = h.get_all_by_label_contains("〈").nth(1).unwrap();
    second.click_modifiers(Modifiers::COMMAND);
    h.run();
    assert_eq!(selected_count(&mut h), 3, "Ctrl+クリックで外れる");
    let second = h.get_all_by_label_contains("〈").nth(1).unwrap();
    second.click_modifiers(Modifiers::COMMAND);
    h.run();
    assert_eq!(selected_count(&mut h), 4, "もう一度で戻る");
}

#[test]
fn library_list_opens_templates_and_guards_changes() {
    let tmp = TempDir::new("app-lib-list");
    write_template(tmp.path(), "TEMP1x3_V", "甲");
    write_template(tmp.path(), "TEMP2x1_H", "乙");
    let mut h = harness_with(Some(tmp.path().to_path_buf()), None);
    // 見出し（フォルダのレイアウト）は一覧に出る。状態欄にも同じ文言が出るので件数で見る
    assert!(h.get_all_by_label_contains("横2×縦1（横向き）").count() >= 1);
    h.get_by_label_contains("甲").click();
    h.run();
    assert_eq!(app(&mut h).doc.name, "甲");
    // 開いている物をもう一度押しても何も起きない（状態欄にも同じ名前が出るので全部押す）
    for node in h.get_all_by_label_contains("甲") {
        node.click();
    }
    h.run();
    assert!(app(&mut h).modal.is_none());
    // 変更があれば確認が先
    app(&mut h).doc.checkpoint(None);
    h.get_by_label_contains("乙").click();
    h.run();
    assert!(is_confirm(&h));
    assert_eq!(app(&mut h).doc.name, "甲");
}

#[test]
fn library_without_expinf_is_flagged() {
    let tmp = TempDir::new("app-lib-inf");
    write_template(tmp.path(), "TEMP1x3_V", "甲");
    let h = harness_with(Some(tmp.path().to_path_buf()), None);
    h.get_by_label_contains("expinf が無いフォルダです");
    std::fs::write(tmp.path().join("expinf"), crate::tpl::EXPINF_TEXT).unwrap();
    let h = harness_with(Some(tmp.path().to_path_buf()), None);
    assert!(h.query_by_label_contains("expinf が無い").is_none());
}

#[test]
fn right_panel_shows_card_or_object_properties() {
    let mut h = harness();
    h.get_by_label("カード全体");
    app(&mut h).doc.selection = [0].into();
    h.run();
    h.get_by_label_contains("カード内の 1 枚目の写真です");
    assert!(
        h.get_all_by_label("配置").count() >= 2,
        "メニューと右の欄の見出し"
    );
    assert_eq!(
        h.state().doc.template.objects[0].kind,
        KIND_PICTURE,
        "先頭は写真枠"
    );
}

#[test]
fn status_bar_reflects_state() {
    let mut h = harness();
    assert!(h.query_by_label_contains("未保存").is_none());
    app(&mut h).doc.checkpoint(None);
    app(&mut h).canvas.status = Some("移動 +1.0 mm, +0.0 mm".into());
    h.run();
    h.get_by_label_contains("（未保存）");
    h.get_by_label_contains("移動 +1.0 mm");
}

#[test]
fn canvas_context_actions_are_applied() {
    let mut h = harness();
    app(&mut h).doc.selection = [1].into();
    // 右クリックの結果は CanvasAction 経由で反映される。ここでは関連メソッドの結果を確かめる
    let before = app(&mut h).doc.template.objects.len();
    app(&mut h).duplicate();
    assert_eq!(app(&mut h).doc.template.objects.len(), before + 1);
    app(&mut h).doc.reorder(true);
    let last = app(&mut h).doc.template.objects.len() - 1;
    assert_eq!(app(&mut h).doc.selection, [last].into());
}

#[test]
fn layer_list_shows_fixed_text_not_value_placeholder() {
    // is_value = -1（固定文字）は項目名が空。値の枠（〈〉）ではなく文字そのものを並べる（#22）
    let mut h = harness();
    let mut c = crate::tpl::CaptionBody::new("撮影日", "", false, 12);
    c.is_value = -1;
    app(&mut h)
        .doc
        .template
        .objects
        .push(crate::tpl::Object::new(
            crate::tpl::Body::Caption(c),
            [0.0, 0.0, 1.0, 1.0],
        ));
    h.run();
    assert!(h.query_by_label_contains("撮影日").is_some());
    assert_eq!(
        h.get_all_by_label_contains("〈").count(),
        5,
        "値の枠は増えない"
    );
}

#[test]
fn insert_table_dialog_edits_the_rows_and_arms_the_table_tool() {
    let mut h = harness();
    open_menu(&mut h, "編集", "表組の挿入…");
    assert!(matches!(app(&mut h).modal, Some(Modal::InsertTable { .. })));
    let fields = |h: &mut H| match &app(h).modal {
        Some(Modal::InsertTable { spec }) => spec.fields.clone(),
        _ => panic!("表組の挿入が開いていない"),
    };
    // 最初は標準の 6 行
    assert_eq!(fields(&mut h).len(), 6);
    assert_eq!(fields(&mut h)[0], "工種");
    // 2 行目を下へ（種別 ↔ 細別）、1 行目を削除、行を追加
    h.get_by_label("2 行目を下へ").click();
    h.run();
    assert_eq!(fields(&mut h)[1], "細別");
    assert_eq!(fields(&mut h)[2], "種別");
    h.get_by_label("1 行目を削除").click();
    h.run();
    assert_eq!(fields(&mut h).len(), 5);
    assert_eq!(fields(&mut h)[0], "細別");
    h.get_by_label("行を追加").click();
    h.run();
    // 最後（撮影年月日）の次の項目が入る
    assert_eq!(fields(&mut h).len(), 6);
    assert_eq!(fields(&mut h)[5], "請負者説明文");
    // 置く場所を決める → 表組ツールになり、内容が渡る
    h.get_by_label("置く場所を決める").click();
    h.run();
    assert!(app(&mut h).modal.is_none());
    assert_eq!(app(&mut h).tool, Tool::Table);
    assert_eq!(app(&mut h).canvas.table.fields.len(), 6);
    assert_eq!(app(&mut h).canvas.table.fields[0], "細別");
}

#[test]
fn insert_table_dialog_presets_cancel_and_row_limits() {
    let mut h = harness();
    // ツールバーの「表組…」から開く。「やめる」ではツールは変わらない
    h.get_by_label("▦ 表組…").click();
    h.run();
    h.get_by_label("やめる").click();
    h.run();
    assert!(app(&mut h).modal.is_none());
    assert_eq!(app(&mut h).tool, Tool::Select);
    // プリセットを選ぶと、行が入れ替わる
    h.get_by_label("▦ 表組…").click();
    h.run();
    h.get_by_label("プリセット（実物のテンプレートによくある並び）")
        .click();
    h.run();
    h.get_by_label("工種・種別・写真タイトル").click();
    h.run();
    match &app(&mut h).modal {
        Some(Modal::InsertTable { spec }) => {
            assert_eq!(spec.fields, ["工種", "種別", "写真タイトル"])
        }
        _ => panic!("表組の挿入が開いていない"),
    }
    // 行を全部消すと、置けない
    for _ in 0..3 {
        h.get_by_label("1 行目を削除").click();
        h.run();
    }
    h.get_by_label("行を追加").click();
    h.run();
    h.get_by_label("1 行目を削除").click();
    h.run();
    assert!(
        h.query_by_label("1 行目を削除").is_none(),
        "行が無いので行の操作も無い"
    );
    h.get_by_label("置く場所を決める").click();
    h.run();
    assert!(
        matches!(app(&mut h).modal, Some(Modal::InsertTable { .. })),
        "項目が 0 のときは置く操作を受け付けない"
    );
}

#[test]
fn insert_table_buttons_stay_reachable_with_many_rows_at_the_minimum_window_size() {
    // 最小のウィンドウ（960×600）で 20 行にしても、「置く場所を決める」「やめる」が画面の中に残る。
    // 本番と同じ日本語フォントで測る（フォントの高さで、行の一覧の高さが変わるため）
    let size = egui::vec2(960.0, 600.0);
    let mut h = Harness::builder()
        .with_size(size)
        .allow_missing_glyphs()
        .with_accessibility_check(true)
        .with_max_steps(30)
        .build_eframe(move |cc| {
            crate::install_fonts(&cc.egui_ctx);
            StudioApp::new(&cc.egui_ctx, None, None, Box::new(MemoryStore::default()))
        });
    h.run();
    app(&mut h).canvas.table.fields = (0..20).map(|_| "工種".to_string()).collect();
    open_menu(&mut h, "編集", "表組の挿入…");
    for name in ["置く場所を決める", "やめる"] {
        let r = h.get_by_label(name).rect();
        assert!(
            r.min.y >= 0.0 && r.max.y <= size.y,
            "{name} が画面の外にある: {r:?}"
        );
    }
    // 一覧の下の方の行も、スクロールで操作できる（行が 20 のまま残っている）
    match &app(&mut h).modal {
        Some(Modal::InsertTable { spec }) => assert_eq!(spec.fields.len(), 20),
        _ => panic!("表組の挿入が開いていない"),
    }
}

#[test]
fn align_panel_button_aligns_table_columns_to_the_top_row() {
    let mut h = harness();
    // 見本の 2 行目（項目名 = #4、項目値 = #5）の区切りを、左へずらす
    let x = app(&mut h).doc.template.objects[3].rect[2];
    app(&mut h).doc.template.objects[3].rect[2] = x - 0.3;
    app(&mut h).doc.template.objects[4].rect[0] = x - 0.3;
    app(&mut h).doc.selection = (1..11).collect();
    h.run();
    h.get_by_label("表組の列の区切りをそろえる").click();
    h.run();
    let o = &app(&mut h).doc.template.objects;
    assert!((o[3].rect[2] - o[1].rect[2]).abs() < 1e-9);
    assert!((o[4].rect[0] - o[2].rect[0]).abs() < 1e-9);
    // 2 つ以下しか選んでいないときは押せない
    app(&mut h).doc.selection = [1, 2].into();
    h.run();
    assert!(
        h.get_by_label("表組の列の区切りをそろえる")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn new_template_buttons_stay_reachable_with_the_free_division_inputs_at_the_minimum_window_size() {
    // 分割の絵と「任意」の入力欄が増えても、最小のウィンドウ（960×600）で「作成」「やめる」が画面の中に残る
    let size = egui::vec2(960.0, 600.0);
    let mut h = Harness::builder()
        .with_size(size)
        .allow_missing_glyphs()
        .with_accessibility_check(true)
        .with_max_steps(30)
        .build_eframe(move |cc| {
            crate::install_fonts(&cc.egui_ctx);
            StudioApp::new(&cc.egui_ctx, None, None, Box::new(MemoryStore::default()))
        });
    h.run();
    app(&mut h).run_pending(Pending::New);
    h.run();
    h.get_all_by_label("任意").last().unwrap().click();
    h.run();
    for name in ["作成", "やめる"] {
        let r = h.get_by_label(name).rect();
        assert!(
            r.min.y >= 0.0 && r.max.y <= size.y,
            "{name} が画面の外にある: {r:?}"
        );
    }
}

#[test]
fn new_template_dialog_picks_the_division_by_tile_and_orientation() {
    let mut h = harness();
    app(&mut h).run_pending(Pending::New);
    h.run();
    // 背後のカード全体の欄にも同じ部品があるので、前面の（最後の）ものを操作する
    h.get_all_by_label("横向き").last().unwrap().click();
    h.run();
    h.get_all_by_label("横2×縦3").last().unwrap().click();
    h.run();
    h.get_by_label("作成").click();
    h.run();
    let hd = &app(&mut h).doc.template.header;
    assert_eq!((hd.cols, hd.rows, hd.is_portrait()), (2, 3, false));
}
