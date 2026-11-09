//! 確認・新規作成・名前入力・メッセージの各画面。

use super::*;

impl StudioApp {
    pub(super) fn modals(&mut self, ctx: &egui::Context) {
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
}
