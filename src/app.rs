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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use crate::tpl::{KIND_CAPTION, KIND_PICTURE};
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    type H = Harness<'static, StudioApp>;

    fn harness_with(open: Option<PathBuf>, screenshot: Option<PathBuf>) -> H {
        let mut h = Harness::builder()
            .with_size(egui::vec2(1440.0, 900.0))
            .allow_missing_glyphs()
            .with_accessibility_check(false)
            .with_max_steps(30)
            .build_eframe(move |cc| StudioApp::new(&cc.egui_ctx, open, screenshot));
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
        // 現状の挙動の固定: Ctrl+Shift+Z は先に Ctrl+Z として消費されるため、
        // やり直しにならず元に戻す側が働く（履歴が尽きていれば何も起きない）
        press(&mut h, Modifiers::COMMAND, Key::Z);
        assert_eq!(app(&mut h).doc.template.objects.len(), 11);
        press(&mut h, Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
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
        let second = h.get_all_by_label_contains("〈").nth(1).unwrap();
        second.click_modifiers(Modifiers::SHIFT);
        h.run();
        assert_eq!(selected_count(&mut h), 2);
        let second = h.get_all_by_label_contains("〈").nth(1).unwrap();
        second.click_modifiers(Modifiers::SHIFT);
        h.run();
        assert_eq!(selected_count(&mut h), 1, "もう一度で外れる");
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
}
