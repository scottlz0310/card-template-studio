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
                    let name_label = ui.label("テンプレート名");
                    ui.add(egui::TextEdit::singleline(&mut name).desired_width(f32::INFINITY))
                        .labelled_by(name_label.id);
                    ui.add_space(6.0);
                    ui.label("カードのレイアウト（1 ページの分割数と用紙の向き）");
                    layout_picker::show(ui, "new-layout", &mut layout);
                    ui.add_space(6.0);
                    ui.radio_value(
                        &mut starter,
                        true,
                        "写真枠と「工種・種別・写真タイトル…」の行を置いて始める",
                    );
                    ui.radio_value(&mut starter, false, "白紙から始める");
                    if let Some(e) = &error {
                        ui.colored_label(crate::theme::Colors::of_ui(ui).error, e);
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
                    let name_label = ui.label("保存するテンプレートの名前を入力してください");
                    ui.add(egui::TextEdit::singleline(&mut name).desired_width(320.0))
                        .labelled_by(name_label.id);
                    if let Some(e) = &error {
                        ui.colored_label(crate::theme::Colors::of_ui(ui).error, e);
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
            Modal::InsertTable { mut spec } => {
                let mut place = false;
                egui::Modal::new(egui::Id::unique("table")).show(ctx, |ui| {
                    ui.set_width(460.0);
                    ui.heading("表組の挿入");
                    ui.label("上から並べる項目を決めて「置く場所を決める」を押し、カード上をドラッグした範囲に等分して置きます。");
                    ui.add_space(6.0);
                    let preset_label = ui.label("プリセット（実物のテンプレートによくある並び）");
                    egui::ComboBox::from_id_salt("table-preset")
                        .width(ui.available_width() - 24.0)
                        .selected_text("選ぶと、下の項目をこの並びに入れ替えます")
                        .show_ui(ui, |ui| {
                            for (name, fields) in doc::TABLE_PRESETS {
                                if ui.selectable_label(false, *name).clicked() {
                                    spec.fields = fields.iter().map(|f| (*f).to_string()).collect();
                                }
                            }
                        })
                        .response
                        .labelled_by(preset_label.id);
                    ui.add_space(6.0);
                    let n = spec.fields.len();
                    let mut op: Option<(usize, i32)> = None;
                    // 行が多くても、確定・取消のボタンが画面の外に出ないよう、一覧は高さを抑えてスクロールさせる
                    let rows_height = (ctx.content_rect().height() - 340.0).clamp(80.0, 520.0);
                    egui::ScrollArea::vertical()
                        .max_height(rows_height)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                    for i in 0..n {
                        ui.horizontal(|ui| {
                            let row = ui.label(format!("{} 行目", i + 1));
                            egui::ComboBox::from_id_salt(("table-field", i))
                                .width(200.0)
                                .selected_text(spec.fields[i].clone())
                                .show_ui(ui, |ui| {
                                    for f in doc::KNOWN_FIELDS {
                                        ui.selectable_value(&mut spec.fields[i], (*f).to_string(), *f);
                                    }
                                })
                                .response
                                .labelled_by(row.id);
                            // 同じ「↑」が並ぶので、行ごとの名前を付けて、押したかどうかを返す
                            let named = |r: egui::Response, name: String| {
                                let clicked = r.clicked();
                                r.widget_info(move || {
                                    egui::WidgetInfo::labeled(egui::Role::Button, true, name.clone())
                                });
                                clicked
                            };
                            if named(ui.add_enabled(i > 0, egui::Button::new("↑")), format!("{} 行目を上へ", i + 1)) {
                                op = Some((i, -1));
                            }
                            if named(ui.add_enabled(i + 1 < n, egui::Button::new("↓")), format!("{} 行目を下へ", i + 1)) {
                                op = Some((i, 1));
                            }
                            if named(ui.button("✕"), format!("{} 行目を削除", i + 1)) {
                                op = Some((i, 0));
                            }
                        });
                    }
                        });
                    match op {
                        Some((i, 0)) => {
                            spec.fields.remove(i);
                        }
                        Some((i, d)) => spec.fields.swap(i, (i as i32 + d) as usize),
                        None => {}
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(spec.fields.len() < TABLE_MAX_ROWS, egui::Button::new("行を追加"))
                            .clicked()
                        {
                            let next = spec
                                .fields
                                .last()
                                .map(|l| doc::next_row_field(l).to_string())
                                .unwrap_or_else(|| doc::KNOWN_FIELDS[0].to_string());
                            spec.fields.push(next);
                        }
                        let pct = ui.label("項目名の列の幅");
                        ui.add(
                            egui::DragValue::new(&mut spec.label_pct)
                                .range(10..=60)
                                .suffix(" %"),
                        )
                        .labelled_by(pct.id);
                    });
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(!spec.fields.is_empty(), egui::Button::new("置く場所を決める"))
                            .clicked()
                        {
                            place = true;
                            keep = false;
                        }
                        if ui.button("やめる").clicked() {
                            keep = false;
                        }
                    });
                });
                if place {
                    self.canvas.table = spec;
                    self.tool = Tool::Table;
                } else if keep {
                    next = Some(Modal::InsertTable { spec });
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
