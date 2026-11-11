//! メニューバー、ツールバー、配置のボタン、ヘルプの文。

use super::*;

impl StudioApp {
    pub(super) fn menu_bar(&mut self, ui: &mut egui::Ui) {
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
                {
                    self.open_library_dialog(Some("TMPEXP フォルダを選んでください"));
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
                let can_add_row = self.doc.can_add_row_below();
                if ui
                    .add_enabled(
                        can_add_row,
                        egui::Button::new("下に行を追加").shortcut_text("Ctrl+Enter"),
                    )
                    .clicked()
                {
                    self.doc.add_row_below();
                }
                if ui.button("表組の挿入…").clicked() {
                    self.open_insert_table();
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
                            .range(GRID_MM_RANGE)
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

    pub(super) fn align_buttons(&mut self, ui: &mut egui::Ui) {
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
        ui.add_enabled_ui(n >= 4, |ui| {
            if ui
                .button("表組の列の区切りをそろえる")
                .on_hover_text(
                    "選んだ行を、いちばん上の行の列の区切り（項目名と項目値の境）にそろえます",
                )
                .clicked()
            {
                self.doc.align_table_columns();
            }
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

    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
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
            let table_on = self.tool == Tool::Table;
            let text = RichText::new("▦ 表組…").color(if table_on {
                Color32::WHITE
            } else {
                ui.visuals().text_color()
            });
            let b = egui::Button::new(text)
                .fill(if table_on { ACCENT } else { Color32::TRANSPARENT })
                .corner_radius(6.0);
            if ui
                .add(b)
                .on_hover_text("行数と項目を決めて、カード上をドラッグして表組を置きます")
                .clicked()
            {
                self.open_insert_table();
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
}

const HELP: &str = "■ 本体との受け渡し
・本体のアルバム作成ツールで「カード」→「テンプレートのエクスポート」を行うと TMPEXP フォルダができます。
・このアプリの「ファイル」→「エクスポートしたフォルダ（TMPEXP）を開く」で一覧が出ます。
・編集したら「本体用に書き出す」で新しい TMPEXP フォルダを作り、本体の「テンプレートのインポート」で取り込みます。

■ 編集
・上の道具で写真枠・項目名・項目値・項目の行を選び、カード上をドラッグして置きます。
・部品はドラッグで移動、四隅と辺の□で大きさを変えます。Shift で縦横に固定、Alt を押しながらで複製。
・ピンクの線は、ほかの部品やカードの端・中央にそろったことを示します。
・行（項目名と項目値）を選んで Ctrl+Enter（編集メニューの「下に行を追加」）で、同じ形の行が真下に足されます。項目は 工種 → 種別 → 細別 → 写真タイトル → 撮影箇所 → 撮影年月日 の順に進みます。
・編集メニューの「表組の挿入…」（ツールバーの「▦ 表組…」）で、項目と行数を決めてから、カード上をドラッグした範囲に表組をまとめて置けます。プリセットは実物のテンプレートによくある並びです。
・表組の行を選んで「配置」の「表組の列の区切りをそろえる」を押すと、いちばん上の行の列の区切り（項目名と項目値の境）に、ほかの行がそろいます。複数の文字項目を選ぶと、枠線の有無・色・太さもまとめて変えられます。
・矢印キーで 0.5 mm ずつ（Shift で 5 mm）動かせます。Ctrl+ホイールで拡大縮小、中ボタンかスペース＋ドラッグで画面を動かします。

■ 保持のみ（🔒）の部品
・本体で作った図形・参考図などは、元のバイト列をそのまま保持して書き戻します（このアプリでは変更できません）。";
