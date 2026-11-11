//! アプリ全体：メニュー、道具、左右の欄、ファイル操作、確認の画面。

use crate::canvas::{
    self, ACCENT, CanvasAction, CanvasState, GRID_MM_RANGE, TableSpec, Tool, View,
};
use crate::doc::{self, Align, ClickMods, Document, Library, validate_name};
use crate::layout::layout_label;
use crate::layout_picker;
use crate::props;
use crate::settings::{Settings, SettingsStore, ViewSettings};
use crate::tpl::{Header, MM_PER_INCH, Object, Template};
use dialogs::{Dialogs, NativeDialogs};
use egui::{Color32, Key, KeyboardShortcut, Modifiers, RichText};
use screenshot::Shot;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[cfg(test)]
mod dialog_tests;
mod dialogs;
mod files;
mod menu;
mod modals;
mod panels;
mod screenshot;
mod shortcuts;
#[cfg(test)]
mod tests;

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
    InsertTable {
        spec: TableSpec,
    },
}

/// 表組の行数の上限
const TABLE_MAX_ROWS: usize = 20;

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
    dialogs: Box<dyn Dialogs>,
    /// 保存する設定（最後に使ったフォルダ、表示の設定）。表示の設定は、毎フレーム `view` から取り込む
    settings: Settings,
    settings_store: Box<dyn SettingsStore>,
    /// 最後に保存を試みた内容。これと違うときだけ保存する（失敗した内容を、毎フレーム試し直さない）
    settings_attempted: Settings,
    /// 設定の読み書きの失敗を、利用者に知らせ済みか。保存が成功すると戻す（失敗のたびに知らせない）
    settings_error_shown: bool,
}

impl StudioApp {
    pub fn new(
        ctx: &egui::Context,
        open: Option<PathBuf>,
        screenshot: Option<PathBuf>,
        settings_store: Box<dyn SettingsStore>,
    ) -> Self {
        Self::with_dialogs(
            ctx,
            open,
            screenshot,
            Box::new(NativeDialogs),
            settings_store,
        )
    }

    fn with_dialogs(
        ctx: &egui::Context,
        open: Option<PathBuf>,
        screenshot: Option<PathBuf>,
        dialogs: Box<dyn Dialogs>,
        settings_store: Box<dyn SettingsStore>,
    ) -> Self {
        style(ctx);
        let (settings, settings_error) = match settings_store.load() {
            Ok(s) => (s, None),
            Err(e) => (Settings::default(), Some(e)),
        };
        let mut view = View::default();
        settings.view.apply_to(&mut view);
        let mut app = Self {
            doc: Document::new(
                doc::starter_template(Header::default()),
                "新規テンプレート",
                None,
            ),
            library: None,
            view,
            canvas: CanvasState::default(),
            tool: Tool::Select,
            clipboard: Vec::new(),
            modal: None,
            toast: None,
            shot: None,
            allow_close: false,
            last_title: String::new(),
            dialogs,
            settings_attempted: settings.clone(),
            settings,
            settings_store,
            settings_error_shown: false,
        };
        if let Some(p) = open {
            if p.is_dir() {
                app.open_library(p);
            } else {
                app.open_file(p);
            }
        }
        if let Some(e) = settings_error {
            // 起動時の「開けませんでした」などの表示を、消さない
            let body = format!("{e}\n\n既定の設定で起動しました。");
            if app.modal.is_none() {
                app.error("設定を読めませんでした", body);
            } else {
                app.toast(format!("設定を読めませんでした: {body}"), ctx);
            }
            app.settings_error_shown = true;
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

    /// 設定が変わっていたら保存する。変更は、トグルやダイアログの結果など 1 回ずつなので、すぐ書く。
    /// 失敗は 1 回だけ知らせ、同じ内容では試し直さない（成功すると、次の失敗をまた知らせる）。
    /// 確認用の画面（`--screenshot`）は、利用者の設定を書き換えない。
    fn sync_settings(&mut self, ctx: &egui::Context) {
        if self.shot.is_some() {
            return;
        }
        self.settings.view = ViewSettings::of(&self.view);
        if self.settings == self.settings_attempted {
            return;
        }
        self.settings_attempted = self.settings.clone();
        match self.settings_store.save(&self.settings) {
            Ok(()) => self.settings_error_shown = false,
            Err(e) if !self.settings_error_shown => {
                self.settings_error_shown = true;
                self.toast(format!("設定を保存できませんでした: {e}"), ctx);
            }
            Err(_) => {}
        }
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

    /// 「表組の挿入」を開く。前に決めた内容から始める
    pub(super) fn open_insert_table(&mut self) {
        if self.modal.is_none() {
            self.modal = Some(Modal::InsertTable {
                spec: self.canvas.table.clone(),
            });
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

        if let Some(msg) = self.doc.notice.take() {
            self.toast(msg, &ctx);
        }

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
        self.sync_settings(&ctx);
        self.screenshot_tick(&ctx);
    }

    /// egui の記憶（パネルの幅など）は保存しない。保存するのは、設定として合意した項目だけ
    fn persist_egui_memory(&self) -> bool {
        false
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
