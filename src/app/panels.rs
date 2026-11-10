//! 左の欄（一覧）、右の欄（プロパティ）、状態バー。

use super::*;

impl StudioApp {
    pub(super) fn left_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.label(RichText::new("テンプレート").strong().color(ACCENT));
        ui.separator();
        match self.library.clone() {
            None => {
                ui.label(RichText::new("本体の「カード」メニュー →「テンプレートのエクスポート」で作った TMPEXP フォルダを開くと、ここに一覧が出ます。").small());
                if ui.button("📂 TMPEXP フォルダを開く…").clicked() {
                    self.open_library_dialog(None);
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
                            if c.is_value_cell() {
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

    pub(super) fn right_panel(&mut self, ui: &mut egui::Ui) {
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

    pub(super) fn status_bar(&mut self, ui: &mut egui::Ui) {
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
}
