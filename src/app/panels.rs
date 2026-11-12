//! 左の欄（一覧）、右の欄（プロパティ）、状態バー。

use super::*;

impl StudioApp {
    pub(super) fn left_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        let accent = crate::theme::Colors::of_ui(ui).accent_text;
        ui.label(RichText::new("テンプレート").strong().color(accent));
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
        ui.label(RichText::new("部品の一覧").strong().color(accent));
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("layers")
            .show(ui, |ui| self.layer_list(ui));
    }

    /// 部品の一覧。位置順（上→下、左→右）で、同じ行に並ぶ文字項目は「項目名 ｜ 値」の 1 行にまとめる。
    /// 項目名・値のどちらかをクリックするとその部品だけを、行番号と「｜」をクリックすると行全体を選ぶ
    fn layer_list(&mut self, ui: &mut egui::Ui) {
        let entries = self.doc.layer_entries();
        let groups: Vec<Vec<usize>> = entries.iter().map(LayerEntry::members).collect();
        let mods = ui.input(|i| ClickMods {
            shift: i.modifiers.shift,
            toggle: i.modifiers.command,
        });
        let mut clicked: Option<Vec<usize>> = None;
        let mut row_no = 0;
        // 行の幅（内容に引きずられて欄が広がらないよう、余白を見込んで、先に 1 回だけ取る）
        let width = (ui.available_width() - 10.0).max(120.0);
        for e in &entries {
            match e {
                LayerEntry::Part(i) => {
                    let o = &self.doc.template.objects[*i];
                    let lock = if o.is_passthrough() { "🔒 " } else { "" };
                    let sel = self.doc.selection.contains(i);
                    let text = part_text(o);
                    if ui
                        .selectable_label(sel, format!("{lock}{}  {text}", o.kind_label()))
                        .clicked()
                    {
                        clicked = Some(vec![*i]);
                    }
                }
                LayerEntry::Row(members) => {
                    row_no += 1;
                    if let Some(c) = self.layer_row(ui, row_no, members, width) {
                        clicked = Some(c);
                    }
                }
            }
        }
        if let Some(members) = clicked {
            self.doc.click_select_groups(&members, mods, &groups);
        }
    }

    /// 一覧の 1 行（行番号｜項目名｜値…）。クリックされた部品（行全体なら、行の部品すべて）を返す
    fn layer_row(
        &self,
        ui: &mut egui::Ui,
        no: usize,
        members: &[usize],
        width: f32,
    ) -> Option<Vec<usize>> {
        // ボタンの最小幅（文字＋内側の余白）より大きくして、行が指定の幅からはみ出し、欄が広がるのを防ぐ
        const NO_W: f32 = 20.0;
        const SEP_W: f32 = 22.0;
        const SPACING: f32 = 2.0;
        let sel = &self.doc.selection;
        let all = members.iter().all(|m| sel.contains(m));
        let texts: Vec<String> = members
            .iter()
            .map(|&m| part_text(&self.doc.template.objects[m]))
            .collect();
        // 文字数に比例して、セルの幅を分ける（短い項目名に、長い項目値より広い幅を与えない）
        let weights: Vec<f32> = texts
            .iter()
            .map(|t| t.chars().count() as f32 + 2.0)
            .collect();
        let total_w: f32 = weights.iter().sum();
        let cells = members.len() as f32;
        let items = 1.0 + cells + (cells - 1.0);
        let room =
            (width - NO_W - (cells - 1.0) * SEP_W - SPACING * (items - 1.0)).max(40.0 * cells);
        let h = ui.spacing().interact_size.y;
        let mut clicked = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = SPACING;
            ui.spacing_mut().button_padding = egui::vec2(2.0, 1.0);
            let r = ui
                .add_sized([NO_W, h], egui::Button::selectable(all, format!("{no}")))
                .on_hover_text("この行全体を選ぶ");
            r.widget_info(|| {
                egui::WidgetInfo::labeled(egui::Role::Button, true, format!("{no} 行目の全体"))
            });
            if r.clicked() {
                clicked = Some(members.to_vec());
            }
            for (k, &m) in members.iter().enumerate() {
                if k > 0 {
                    let r = ui
                        .add_sized([SEP_W, h], egui::Button::selectable(all, "｜"))
                        .on_hover_text("この行全体を選ぶ");
                    r.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::Role::Button,
                            true,
                            format!("{no} 行目の全体（区切り）"),
                        )
                    });
                    if r.clicked() {
                        clicked = Some(members.to_vec());
                    }
                }
                let text = texts[k].clone();
                let r = ui
                    .add_sized(
                        [room * weights[k] / total_w, h],
                        egui::Button::selectable(
                            sel.contains(&m),
                            (text.clone(), egui::Atom::grow()),
                        )
                        .truncate(),
                    )
                    .on_hover_text(text);
                if r.clicked() {
                    clicked = Some(vec![m]);
                }
            }
        });
        clicked
    }

    pub(super) fn right_panel(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(4.0);
            props::show(ui, &mut self.doc, &mut self.canvas.focus_text);
            if !self.doc.selected_editable().is_empty() {
                ui.add_space(8.0);
                let accent = crate::theme::Colors::of_ui(ui).accent_text;
                ui.label(RichText::new("配置").strong().color(accent));
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

/// 部品の一覧に出す文字（項目値は `〈項目名〉`、項目名・文字はその文字）
fn part_text(o: &crate::tpl::Object) -> String {
    match &o.body {
        crate::tpl::Body::Caption(c) => {
            if c.is_value_cell() {
                format!("〈{}〉", c.field.to_string_lossy())
            } else {
                c.text.to_string_lossy().replace(
                    "
", " ",
                )
            }
        }
        _ => String::new(),
    }
}
