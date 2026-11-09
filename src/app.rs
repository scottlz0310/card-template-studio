//! アプリ全体：メニュー、道具、左右の欄、ファイル操作、確認の画面。

use crate::canvas::{self, ACCENT, CanvasAction, CanvasState, Tool, View};
use crate::doc::{self, Align, Document, Library, validate_name};
use crate::layout::{PRESETS, layout_label};
use crate::props;
use crate::tpl::{Header, MM_PER_INCH, Object, Template};
use egui::{Color32, Key, KeyboardShortcut, Modifiers, RichText};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const MM: f64 = 1.0 / MM_PER_INCH;

enum Pending {
    New,
    Open(PathBuf),
    Close,
}

enum Modal {
    NewTemplate {
        name: String,
        layout: (u16, u16, bool),
        starter: bool,
        error: Option<String>,
    },
    ConfirmDiscard(Pending),
    SaveAsName {
        name: String,
        error: Option<String>,
    },
    Message {
        title: String,
        body: String,
    },
}

struct Shot {
    path: PathBuf,
    frames: u32,
    requested: bool,
    image: Arc<Mutex<Option<Arc<egui::ColorImage>>>>,
}

pub struct StudioApp {
    doc: Document,
    library: Option<Library>,
    view: View,
    canvas: CanvasState,
    tool: Tool,
    clipboard: Vec<Object>,
    modal: Option<Modal>,
    toast: Option<(String, f64)>,
    shot: Option<Shot>,
    allow_close: bool,
    last_title: String,
}

impl StudioApp {
    pub fn new(ctx: &egui::Context, open: Option<PathBuf>, screenshot: Option<PathBuf>) -> Self {
        style(ctx);
        let mut app = Self {
            doc: Document::new(
                doc::starter_template(Header::default()),
                "新規テンプレート",
                None,
            ),
            library: None,
            view: View::default(),
            canvas: CanvasState::default(),
            tool: Tool::Select,
            clipboard: Vec::new(),
            modal: None,
            toast: None,
            shot: None,
            allow_close: false,
            last_title: String::new(),
        };
        if let Some(p) = open {
            if p.is_dir() {
                app.open_library(p);
            } else {
                app.open_file(p);
            }
        }
        if let Some(path) = screenshot {
            // 確認用の画面: ファイルの指定が無ければ見本のカードを開き、項目値を 1 つ選んだ状態を写す
            if app.doc.path.is_none() {
                app.doc = Document::new(
                    doc::starter_template(Header::default()),
                    "見本テンプレート",
                    None,
                );
            }
            if let Some(i) = app
                .doc
                .template
                .objects
                .iter()
                .position(|o| o.kind == crate::tpl::KIND_CAPTION)
            {
                app.doc.selection = [i + 1].into();
            }
            app.shot = Some(Shot {
                path,
                frames: 0,
                requested: false,
                image: Arc::new(Mutex::new(None)),
            });
        }
        app
    }

    fn toast(&mut self, msg: impl Into<String>, ctx: &egui::Context) {
        self.toast = Some((msg.into(), ctx.input(|i| i.time)));
    }

    fn error(&mut self, title: &str, body: impl Into<String>) {
        self.modal = Some(Modal::Message {
            title: title.into(),
            body: body.into(),
        });
    }

    // ----- ファイル -----

    fn guard(&mut self, p: Pending) {
        if self.doc.dirty {
            self.modal = Some(Modal::ConfirmDiscard(p));
        } else {
            self.run_pending(p);
        }
    }

    fn run_pending(&mut self, p: Pending) {
        match p {
            Pending::New => {
                self.modal = Some(Modal::NewTemplate {
                    name: "新規テンプレート".into(),
                    layout: (
                        self.doc.template.header.cols,
                        self.doc.template.header.rows,
                        self.doc.template.header.is_portrait(),
                    ),
                    starter: true,
                    error: None,
                })
            }
            Pending::Open(path) => self.open_file(path),
            Pending::Close => self.allow_close = true,
        }
    }

    fn open_file(&mut self, path: PathBuf) {
        match Document::open(&path) {
            Ok(d) => {
                self.doc = d;
                self.view.fit_pending = true;
                self.tool = Tool::Select;
            }
            Err(e) => self.error("開けませんでした", e),
        }
    }

    fn open_library(&mut self, root: PathBuf) {
        match Library::scan(&root) {
            Ok(lib) if lib.entries.is_empty() => self.error(
                "テンプレートが見つかりません",
                format!("{} の中に「TEMP1x3_V」のようなフォルダと .TPL ファイルが見つかりませんでした。\n本体の「テンプレートのエクスポート」で作った TMPEXP フォルダを選んでください。", root.display()),
            ),
            Ok(lib) => self.library = Some(lib),
            Err(e) => self.error("フォルダを開けません", e),
        }
    }

    fn save(&mut self, ctx: &egui::Context) {
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

    fn save_to(&mut self, path: PathBuf, ctx: &egui::Context) {
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

    fn save_as_dialog(&mut self, ctx: &egui::Context) {
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

    fn export_dialog(&mut self, ctx: &egui::Context, all: bool) {
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

    // ----- 編集 -----

    fn duplicate(&mut self) {
        let objs = self.doc.copy_selected();
        self.doc.paste(&objs, 2.0 * MM);
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.modal.is_some() {
            return;
        }
        let cmd = |k| KeyboardShortcut::new(Modifiers::COMMAND, k);
        let cmd_shift = |k| KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, k);
        let (z, y, sz, s, ss, o, n, e) = ctx.input_mut(|i| {
            (
                i.consume_shortcut(&cmd(Key::Z)),
                i.consume_shortcut(&cmd(Key::Y)),
                i.consume_shortcut(&cmd_shift(Key::Z)),
                i.consume_shortcut(&cmd(Key::S)),
                i.consume_shortcut(&cmd_shift(Key::S)),
                i.consume_shortcut(&cmd(Key::O)),
                i.consume_shortcut(&cmd(Key::N)),
                i.consume_shortcut(&cmd(Key::E)),
            )
        });
        if s {
            self.save(ctx);
        }
        if ss {
            self.save_as_dialog(ctx);
        }
        if o {
            self.open_dialog();
        }
        if n {
            self.guard(Pending::New);
        }
        if e {
            self.export_dialog(ctx, false);
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        if z {
            self.doc.undo();
        }
        if y || sz {
            self.doc.redo();
        }
        let (c, x, v, d, a, del, esc, zero) = ctx.input_mut(|i| {
            (
                i.consume_shortcut(&cmd(Key::C)),
                i.consume_shortcut(&cmd(Key::X)),
                i.consume_shortcut(&cmd(Key::V)),
                i.consume_shortcut(&cmd(Key::D)),
                i.consume_shortcut(&cmd(Key::A)),
                i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace),
                i.key_pressed(Key::Escape),
                i.consume_shortcut(&cmd(Key::Num0)),
            )
        });
        if c {
            self.clipboard = self.doc.copy_selected();
        }
        if x {
            self.clipboard = self.doc.copy_selected();
            self.doc.delete_selected();
        }
        if v {
            let objs = self.clipboard.clone();
            self.doc.paste(&objs, 2.0 * MM);
        }
        if d {
            self.duplicate();
        }
        if a {
            self.doc.select_all();
        }
        if del {
            self.doc.delete_selected();
        }
        if esc {
            self.doc.selection.clear();
            self.tool = Tool::Select;
        }
        if zero {
            self.view.fit_pending = true;
        }
        let tools = [
            (Key::V, Tool::Select),
            (Key::P, Tool::Picture),
            (Key::T, Tool::Label),
            (Key::D, Tool::Value),
            (Key::R, Tool::Row),
        ];
        for (k, t) in tools {
            if ctx.input(|i| i.modifiers.is_none() && i.key_pressed(k)) {
                self.tool = t;
            }
        }
        // 矢印キーで少しずつ動かす（Shift で 5 mm）
        let (dx, dy, fast) = ctx.input(|i| {
            let mut dx = 0.0;
            let mut dy = 0.0;
            if i.key_pressed(Key::ArrowLeft) {
                dx -= 1.0;
            }
            if i.key_pressed(Key::ArrowRight) {
                dx += 1.0;
            }
            if i.key_pressed(Key::ArrowUp) {
                dy -= 1.0;
            }
            if i.key_pressed(Key::ArrowDown) {
                dy += 1.0;
            }
            (dx, dy, i.modifiers.shift)
        });
        if dx != 0.0 || dy != 0.0 {
            let step = if fast { 5.0 } else { 0.5 } * MM;
            self.doc.move_selected(dx * step, dy * step, Some("nudge"));
        } else if ctx.input(|i| i.keys_down.is_empty()) {
            // 矢印を離したら次の移動は別の「元に戻す」にする
        }
    }

    fn open_dialog(&mut self) {
        let mut dlg = rfd::FileDialog::new().add_filter("カードテンプレート", &["TPL", "tpl"]);
        if let Some(lib) = &self.library {
            dlg = dlg.set_directory(&lib.root);
        }
        if let Some(p) = dlg.pick_file() {
            self.guard(Pending::Open(p));
        }
    }

    // ----- 画面 -----

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("ファイル", |ui| {
                if ui
                    .add(egui::Button::new("新規テンプレート…").shortcut_text("Ctrl+N"))
                    .clicked()
                {
                    self.guard(Pending::New);
                }
                if ui
                    .add(egui::Button::new("テンプレートを開く…").shortcut_text("Ctrl+O"))
                    .clicked()
                {
                    self.open_dialog();
                }
                if ui
                    .button("エクスポートしたフォルダ（TMPEXP）を開く…")
                    .clicked()
                    && let Some(p) = rfd::FileDialog::new()
                        .set_title("TMPEXP フォルダを選んでください")
                        .pick_folder()
                {
                    self.open_library(p);
                }
                ui.separator();
                if ui
                    .add(egui::Button::new("上書き保存").shortcut_text("Ctrl+S"))
                    .clicked()
                {
                    self.save(ui.ctx());
                }
                if ui
                    .add(egui::Button::new("名前を付けて保存…").shortcut_text("Ctrl+Shift+S"))
                    .clicked()
                {
                    self.save_as_dialog(ui.ctx());
                }
                ui.separator();
                if ui
                    .add(
                        egui::Button::new("このテンプレートを本体用に書き出す…")
                            .shortcut_text("Ctrl+E"),
                    )
                    .clicked()
                {
                    self.export_dialog(ui.ctx(), false);
                }
                if ui
                    .add_enabled(
                        self.library.is_some(),
                        egui::Button::new("一覧のテンプレートをまとめて書き出す…"),
                    )
                    .clicked()
                {
                    self.export_dialog(ui.ctx(), true);
                }
                ui.separator();
                if ui.button("終了").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("編集", |ui| {
                if ui
                    .add_enabled(
                        self.doc.can_undo(),
                        egui::Button::new("元に戻す").shortcut_text("Ctrl+Z"),
                    )
                    .clicked()
                {
                    self.doc.undo();
                }
                if ui
                    .add_enabled(
                        self.doc.can_redo(),
                        egui::Button::new("やり直し").shortcut_text("Ctrl+Y"),
                    )
                    .clicked()
                {
                    self.doc.redo();
                }
                ui.separator();
                let any = !self.doc.selected_editable().is_empty();
                if ui
                    .add_enabled(any, egui::Button::new("コピー").shortcut_text("Ctrl+C"))
                    .clicked()
                {
                    self.clipboard = self.doc.copy_selected();
                }
                if ui
                    .add_enabled(
                        !self.clipboard.is_empty(),
                        egui::Button::new("貼り付け").shortcut_text("Ctrl+V"),
                    )
                    .clicked()
                {
                    let objs = self.clipboard.clone();
                    self.doc.paste(&objs, 2.0 * MM);
                }
                if ui
                    .add_enabled(any, egui::Button::new("複製").shortcut_text("Ctrl+D"))
                    .clicked()
                {
                    self.duplicate();
                }
                if ui
                    .add_enabled(any, egui::Button::new("削除").shortcut_text("Delete"))
                    .clicked()
                {
                    self.doc.delete_selected();
                }
                ui.separator();
                if ui
                    .add(egui::Button::new("すべて選択").shortcut_text("Ctrl+A"))
                    .clicked()
                {
                    self.doc.select_all();
                }
            });
            ui.menu_button("配置", |ui| self.align_buttons(ui));
            ui.menu_button("表示", |ui| {
                if ui
                    .add(egui::Button::new("カード全体を表示").shortcut_text("Ctrl+0"))
                    .clicked()
                {
                    self.view.fit_pending = true;
                }
                ui.checkbox(&mut self.view.show_grid, "方眼を表示");
                ui.checkbox(&mut self.view.snap_grid, "方眼に吸着");
                ui.checkbox(&mut self.view.snap_objects, "ほかの部品・カードの端に吸着");
                ui.checkbox(
                    &mut self.view.show_used,
                    "既存テンプレートの配置範囲（緑の点線）",
                );
                ui.checkbox(&mut self.view.sample_values, "項目値に見本の文字を出す");
                ui.horizontal(|ui| {
                    ui.label("方眼の間隔");
                    ui.add(
                        egui::DragValue::new(&mut self.view.grid_mm)
                            .range(0.5..=20.0)
                            .speed(0.1)
                            .suffix(" mm"),
                    );
                });
            });
            ui.menu_button("ヘルプ", |ui| {
                if ui.button("使い方").clicked() {
                    self.modal = Some(Modal::Message {
                        title: "使い方".into(),
                        body: HELP.into(),
                    });
                }
            });
        });
    }

    fn align_buttons(&mut self, ui: &mut egui::Ui) {
        let n = self.doc.selected_editable().len();
        ui.add_enabled_ui(n >= 2, |ui| {
            ui.horizontal(|ui| {
                for (t, a, tip) in [
                    ("⇤", Align::Left, "左揃え"),
                    ("⇹", Align::CenterH, "左右中央揃え"),
                    ("⇥", Align::Right, "右揃え"),
                ] {
                    if ui.button(t).on_hover_text(tip).clicked() {
                        self.doc.align(a);
                    }
                }
                for (t, a, tip) in [
                    ("⤒", Align::Top, "上揃え"),
                    ("⇳", Align::CenterV, "上下中央揃え"),
                    ("⤓", Align::Bottom, "下揃え"),
                ] {
                    if ui.button(t).on_hover_text(tip).clicked() {
                        self.doc.align(a);
                    }
                }
            });
            ui.horizontal(|ui| {
                if ui
                    .button("幅を揃える")
                    .on_hover_text("最初に選んだ部品の幅に揃えます")
                    .clicked()
                {
                    self.doc.match_size(true, false);
                }
                if ui.button("高さを揃える").clicked() {
                    self.doc.match_size(false, true);
                }
            });
        });
        ui.add_enabled_ui(n >= 3, |ui| {
            ui.horizontal(|ui| {
                if ui.button("横の間隔を均等").clicked() {
                    self.doc.distribute(true);
                }
                if ui.button("縦の間隔を均等").clicked() {
                    self.doc.distribute(false);
                }
            });
        });
        ui.add_enabled_ui(n >= 1, |ui| {
            ui.horizontal(|ui| {
                if ui.button("カードの左右中央へ").clicked() {
                    self.doc.align(Align::CardCenterH);
                }
                if ui.button("上下中央へ").clicked() {
                    self.doc.align(Align::CardCenterV);
                }
            });
            ui.horizontal(|ui| {
                if ui.button("最前面へ").clicked() {
                    self.doc.reorder(true);
                }
                if ui.button("最背面へ").clicked() {
                    self.doc.reorder(false);
                }
            });
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (t, icon, key) in [
                (Tool::Select, "⬉", "V"),
                (Tool::Picture, "🖼", "P"),
                (Tool::Label, "Ａ", "T"),
                (Tool::Value, "▭", "D"),
                (Tool::Row, "☰", "R"),
            ] {
                let selected = self.tool == t;
                let text = RichText::new(format!("{icon} {}", t.label())).color(if selected { Color32::WHITE } else { ui.visuals().text_color() });
                let b = egui::Button::new(text).fill(if selected { ACCENT } else { Color32::TRANSPARENT }).corner_radius(6.0);
                if ui.add(b).on_hover_text(format!("{}（{key}）", t.hint())).clicked() {
                    self.tool = t;
                }
            }
            ui.separator();
            if ui.add_enabled(self.doc.can_undo(), egui::Button::new("↶ 元に戻す")).clicked() {
                self.doc.undo();
            }
            if ui.add_enabled(self.doc.can_redo(), egui::Button::new("↷ やり直し")).clicked() {
                self.doc.redo();
            }
            ui.separator();
            if ui.button("－").on_hover_text("縮小").clicked() {
                self.view.zoom = (self.view.zoom / 1.25).max(0.2);
            }
            ui.label(format!("{:.0}%", self.view.zoom * 100.0));
            if ui.button("＋").on_hover_text("拡大").clicked() {
                self.view.zoom = (self.view.zoom * 1.25).min(8.0);
            }
            if ui.button("全体").on_hover_text("カード全体を表示（Ctrl+0）").clicked() {
                self.view.fit_pending = true;
            }
            ui.separator();
            ui.toggle_value(&mut self.view.snap_grid, "方眼吸着");
            ui.toggle_value(&mut self.view.snap_objects, "ガイド吸着");
            ui.toggle_value(&mut self.view.sample_values, "見本表示");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(RichText::new("⤴ 本体用に書き出す").color(Color32::WHITE)).fill(Color32::from_rgb(0x2e, 0x7d, 0x57)).corner_radius(6.0)).on_hover_text("TMPEXP フォルダを作り、本体の「テンプレートのインポート」で取り込める形で保存します").clicked() {
                    self.export_dialog(ui.ctx(), false);
                }
                if ui.add(egui::Button::new("💾 保存").corner_radius(6.0)).clicked() {
                    self.save(ui.ctx());
                }
            });
        });
    }

    fn left_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.label(RichText::new("テンプレート").strong().color(ACCENT));
        ui.separator();
        match self.library.clone() {
            None => {
                ui.label(RichText::new("本体の「カード」メニュー →「テンプレートのエクスポート」で作った TMPEXP フォルダを開くと、ここに一覧が出ます。").small());
                if ui.button("📂 TMPEXP フォルダを開く…").clicked()
                    && let Some(p) = rfd::FileDialog::new().pick_folder()
                {
                    self.open_library(p);
                }
            }
            Some(lib) => {
                ui.label(RichText::new(lib.root.display().to_string()).small().weak());
                if !lib.is_export {
                    ui.label(
                        RichText::new("（expinf が無いフォルダです）")
                            .small()
                            .weak(),
                    );
                }
                let h = ui.available_height() * 0.45;
                egui::ScrollArea::vertical()
                    .id_salt("lib")
                    .max_height(h)
                    .show(ui, |ui| {
                        let mut current_folder = String::new();
                        for e in &lib.entries {
                            if e.folder != current_folder {
                                current_folder = e.folder.clone();
                                let label = crate::tpl::parse_folder_name(&e.folder)
                                    .map(|(c, r, o)| layout_label(c, r, o != 0))
                                    .unwrap_or_else(|| e.folder.clone());
                                ui.add_space(4.0);
                                ui.label(RichText::new(label).small().strong());
                            }
                            let open = self.doc.path.as_deref() == Some(e.path.as_path());
                            if ui.selectable_label(open, format!("  {}", e.name)).clicked() && !open
                            {
                                self.guard(Pending::Open(e.path.clone()));
                            }
                        }
                    });
            }
        }
        ui.add_space(8.0);
        ui.label(RichText::new("部品の一覧").strong().color(ACCENT));
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("layers")
            .show(ui, |ui| {
                let mut clicked: Option<(usize, bool)> = None;
                let mut picture_no = 0;
                for (i, o) in self.doc.template.objects.iter().enumerate().rev() {
                    let _ = picture_no;
                    let text = match &o.body {
                        crate::tpl::Body::Caption(c) => {
                            if c.is_value != 0 {
                                format!("〈{}〉", c.field.to_string_lossy())
                            } else {
                                c.text.to_string_lossy().replace("\r\n", " ")
                            }
                        }
                        crate::tpl::Body::Picture(_) => {
                            picture_no += 1;
                            String::new()
                        }
                        _ => String::new(),
                    };
                    let lock = if o.is_passthrough() { "🔒 " } else { "" };
                    let sel = self.doc.selection.contains(&i);
                    let r = ui.selectable_label(sel, format!("{lock}{}  {}", o.kind_label(), text));
                    if r.clicked() {
                        clicked = Some((i, ui.input(|i| i.modifiers.shift || i.modifiers.command)));
                    }
                }
                if let Some((i, add)) = clicked {
                    if add {
                        if !self.doc.selection.remove(&i) {
                            self.doc.selection.insert(i);
                        }
                    } else {
                        self.doc.selection = [i].into();
                    }
                }
            });
    }

    fn right_panel(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(4.0);
            props::show(ui, &mut self.doc, &mut self.canvas.focus_text);
            if !self.doc.selected_editable().is_empty() {
                ui.add_space(8.0);
                ui.label(RichText::new("配置").strong().color(ACCENT));
                ui.separator();
                self.align_buttons(ui);
            }
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let name = format!(
                "{}{}",
                self.doc.name,
                if self.doc.dirty {
                    "（未保存）"
                } else {
                    ""
                }
            );
            ui.label(RichText::new(name).strong());
            ui.separator();
            ui.label(layout_label(
                self.doc.template.header.cols,
                self.doc.template.header.rows,
                self.doc.template.header.is_portrait(),
            ));
            ui.separator();
            if let Some((x, y)) = self.canvas.cursor_inch {
                ui.label(format!(
                    "X {:.1} mm  Y {:.1} mm",
                    x * MM_PER_INCH,
                    y * MM_PER_INCH
                ));
                ui.separator();
            }
            let msg = self
                .canvas
                .status
                .clone()
                .unwrap_or_else(|| self.tool.hint().to_string());
            ui.label(RichText::new(msg).weak());
        });
    }

    fn modals(&mut self, ctx: &egui::Context) {
        let Some(modal) = self.modal.take() else {
            return;
        };
        let mut keep = true;
        let mut next: Option<Modal> = None;
        match modal {
            Modal::NewTemplate {
                mut name,
                mut layout,
                mut starter,
                mut error,
            } => {
                egui::Modal::new(egui::Id::unique("new")).show(ctx, |ui| {
                    ui.set_width(380.0);
                    ui.heading("新規テンプレート");
                    ui.add_space(6.0);
                    ui.label("テンプレート名");
                    ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY));
                    ui.add_space(6.0);
                    ui.label("カードのレイアウト（1 ページの分割数と用紙の向き）");
                    egui::ComboBox::from_id_salt("new-layout")
                        .width(260.0)
                        .selected_text(layout_label(layout.0, layout.1, layout.2))
                        .show_ui(ui, |ui| {
                            for &(c, r, p) in PRESETS {
                                ui.selectable_value(&mut layout, (c, r, p), layout_label(c, r, p));
                            }
                        });
                    ui.add_space(6.0);
                    ui.radio_value(
                        &mut starter,
                        true,
                        "写真枠と「工種・種別・写真タイトル…」の行を置いて始める",
                    );
                    ui.radio_value(&mut starter, false, "白紙から始める");
                    if let Some(e) = &error {
                        ui.colored_label(Color32::from_rgb(0xc0, 0x39, 0x2b), e);
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("作成").clicked() {
                            match validate_name(&name) {
                                Ok(()) => {
                                    let header = Header {
                                        cols: layout.0,
                                        rows: layout.1,
                                        orientation: layout.2 as u32,
                                        ..Header::default()
                                    };
                                    let template = if starter {
                                        doc::starter_template(header)
                                    } else {
                                        Template {
                                            header,
                                            ..Default::default()
                                        }
                                    };
                                    let mut d = Document::new(template, name.trim(), None);
                                    d.dirty = true;
                                    self.doc = d;
                                    self.view.fit_pending = true;
                                    keep = false;
                                }
                                Err(e) => error = Some(e),
                            }
                        }
                        if ui.button("やめる").clicked() {
                            keep = false;
                        }
                    });
                });
                if keep {
                    next = Some(Modal::NewTemplate {
                        name,
                        layout,
                        starter,
                        error,
                    });
                }
            }
            Modal::ConfirmDiscard(p) => {
                let mut choice = None;
                egui::Modal::new(egui::Id::unique("discard")).show(ctx, |ui| {
                    ui.heading("保存していない変更があります");
                    ui.label(format!("「{}」の変更を保存しますか？", self.doc.name));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("保存する").clicked() {
                            choice = Some(0);
                        }
                        if ui.button("保存しない").clicked() {
                            choice = Some(1);
                        }
                        if ui.button("やめる").clicked() {
                            choice = Some(2);
                        }
                    });
                });
                match choice {
                    Some(0) => {
                        self.save(ctx);
                        if !self.doc.dirty {
                            self.run_pending(p);
                        }
                    }
                    Some(1) => {
                        self.doc.dirty = false;
                        self.run_pending(p);
                    }
                    Some(_) => {}
                    None => next = Some(Modal::ConfirmDiscard(p)),
                }
            }
            Modal::SaveAsName {
                mut name,
                mut error,
            } => {
                egui::Modal::new(egui::Id::unique("saveas")).show(ctx, |ui| {
                    ui.heading("保存");
                    ui.label("保存するテンプレートの名前を入力してください");
                    ui.add(egui::TextEdit::singleline(&mut name).desired_width(320.0));
                    if let Some(e) = &error {
                        ui.colored_label(Color32::from_rgb(0xc0, 0x39, 0x2b), e);
                    }
                    ui.horizontal(|ui| {
                        if ui.button("保存先を選ぶ…").clicked() {
                            match validate_name(&name) {
                                Ok(()) => {
                                    self.doc.name = name.trim().to_string();
                                    keep = false;
                                    self.save_as_dialog(ctx);
                                }
                                Err(e) => error = Some(e),
                            }
                        }
                        if ui.button("やめる").clicked() {
                            keep = false;
                        }
                    });
                });
                if keep && self.modal.is_none() {
                    next = Some(Modal::SaveAsName { name, error });
                }
            }
            Modal::Message { title, body } => {
                egui::Modal::new(egui::Id::unique("msg")).show(ctx, |ui| {
                    ui.set_max_width(520.0);
                    ui.heading(&title);
                    ui.add_space(4.0);
                    ui.label(&body);
                    ui.add_space(8.0);
                    if ui.button("閉じる").clicked() {
                        keep = false;
                    }
                });
                if keep {
                    next = Some(Modal::Message { title, body });
                }
            }
        }
        if self.modal.is_none() {
            self.modal = next;
        }
    }

    fn screenshot_tick(&mut self, ctx: &egui::Context) {
        let Some(shot) = &mut self.shot else { return };
        shot.frames += 1;
        ctx.request_repaint();
        if shot.frames >= 120 && !shot.requested {
            shot.requested = true;
            let slot = shot.image.clone();
            let ctx2 = ctx.clone();
            ctx.request_screenshot(move |img| {
                *slot.lock().unwrap() = Some(img);
                ctx2.request_repaint();
            });
        }
        let img = shot.image.lock().unwrap().take();
        if let Some(img) = img {
            let [w, h] = img.size;
            let mut rgba = Vec::with_capacity(w * h * 4);
            for p in &img.pixels {
                rgba.extend_from_slice(&p.to_srgba_unmultiplied());
            }
            if let Some(dir) = shot.path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Err(e) = image::save_buffer(
                &shot.path,
                &rgba,
                w as u32,
                h as u32,
                image::ColorType::Rgba8,
            ) {
                eprintln!("スクリーンショットを保存できません: {e}");
            }
            self.allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let title = format!(
            "{}{} - カードテンプレート工房",
            self.doc.name,
            if self.doc.dirty { " *" } else { "" }
        );
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_close
            && self.doc.dirty
            && self.shot.is_none()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.modal.is_none() {
                self.modal = Some(Modal::ConfirmDiscard(Pending::Close));
            }
        }
        if self.allow_close && self.shot.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.shortcuts(&ctx);

        egui::Panel::top("menu").show(ui, |ui| {
            self.menu_bar(ui);
        });
        egui::Panel::top("tools")
            .frame(
                egui::Frame::new()
                    .fill(ui.visuals().panel_fill)
                    .inner_margin(egui::Margin::symmetric(8, 6)),
            )
            .show(ui, |ui| {
                self.toolbar(ui);
            });
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("left")
            .resizable(true)
            .default_size(240.0)
            .size_range(180.0..=420.0)
            .show(ui, |ui| self.left_panel(ui));
        egui::Panel::right("right")
            .resizable(true)
            .default_size(320.0)
            .size_range(260.0..=480.0)
            .show(ui, |ui| self.right_panel(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::new())
            .show(ui, |ui| {
                let action = canvas::show(
                    ui,
                    &mut self.doc,
                    &mut self.view,
                    &mut self.canvas,
                    &mut self.tool,
                );
                match action {
                    CanvasAction::ContextDuplicate => self.duplicate(),
                    CanvasAction::ContextDelete => {
                        self.doc.delete_selected();
                    }
                    CanvasAction::ContextFront => self.doc.reorder(true),
                    CanvasAction::ContextBack => self.doc.reorder(false),
                    CanvasAction::None => {}
                }
            });

        if let Some((msg, t)) = self.toast.clone() {
            let now = ctx.input(|i| i.time);
            if now - t > 6.0 {
                self.toast = None;
            } else {
                egui::Area::new(egui::Id::unique("toast"))
                    .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -48.0])
                    .show(&ctx, |ui| {
                        egui::Frame::popup(ui.style())
                            .fill(Color32::from_rgb(0x24, 0x2b, 0x36))
                            .show(ui, |ui| {
                                ui.set_max_width(640.0);
                                ui.label(RichText::new(msg).color(Color32::WHITE));
                            });
                    });
                ctx.request_repaint_after(std::time::Duration::from_millis(250));
            }
        }
        self.modals(&ctx);
        self.screenshot_tick(&ctx);
    }
}

fn style(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Light);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(8.0, 4.0);
        s.visuals.selection.bg_fill = ACCENT;
        s.visuals.selection.stroke = egui::Stroke::new(1.0, Color32::WHITE);
        s.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(5);
        s.visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(5);
        s.visuals.widgets.active.corner_radius = egui::CornerRadius::same(5);
        s.visuals.panel_fill = Color32::from_rgb(0xf7, 0xf8, 0xfa);
    });
}

const HELP: &str = "■ 本体との受け渡し
・本体のアルバム作成ツールで「カード」→「テンプレートのエクスポート」を行うと TMPEXP フォルダができます。
・このアプリの「ファイル」→「エクスポートしたフォルダ（TMPEXP）を開く」で一覧が出ます。
・編集したら「本体用に書き出す」で新しい TMPEXP フォルダを作り、本体の「テンプレートのインポート」で取り込みます。

■ 編集
・上の道具で写真枠・項目名・項目値・項目の行を選び、カード上をドラッグして置きます。
・部品はドラッグで移動、四隅と辺の□で大きさを変えます。Shift で縦横に固定、Alt を押しながらで複製。
・ピンクの線は、ほかの部品やカードの端・中央にそろったことを示します。
・矢印キーで 0.5 mm ずつ（Shift で 5 mm）動かせます。Ctrl+ホイールで拡大縮小、中ボタンかスペース＋ドラッグで画面を動かします。

■ 保持のみ（🔒）の部品
・本体で作った図形・参考図などは、元のバイト列をそのまま保持して書き戻します（このアプリでは変更できません）。";
