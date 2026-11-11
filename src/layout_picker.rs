//! 1 ページの分割数と用紙の向きを、用紙の絵で選ぶ部品。
//! 新規作成の画面と、カード全体のプロパティで同じものを使う。

use crate::layout::layout_label;
use crate::tpl::{A4_LONG_INCH, A4_SHORT_INCH};
use egui::{Color32, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

/// 横の分割数・縦の分割数・縦向きかどうか
pub type Layout = (u16, u16, bool);

/// 本体の「分割数」メニューにある固定の候補（横×縦）。ここに無い組は「任意」で選ぶ。
pub const FIXED: &[(u16, u16)] = &[(1, 1), (1, 2), (1, 3), (1, 4), (2, 3), (3, 4), (4, 5)];

/// 「任意」で入れられる分割数の上限（本体の上限は未確認なので、実用上十分な値にしている）
pub const CUSTOM_MAX: u16 = 10;

const TILE: egui::Vec2 = vec2(64.0, 80.0);
/// 用紙の絵の長辺
const SHEET_LONG: f32 = 52.0;

/// 選び直したら true を返す。`salt` は同じ画面に 2 つ置くときの区別。
pub fn show(ui: &mut Ui, salt: &str, layout: &mut Layout) -> bool {
    let before = *layout;
    let custom_id = ui.make_persistent_id((salt, "custom"));
    let forced_custom: bool = ui.data(|d| d.get_temp(custom_id)).unwrap_or(false);
    let is_custom = forced_custom || !FIXED.contains(&(layout.0, layout.1));

    ui.horizontal(|ui| {
        ui.label("用紙の向き");
        for (portrait, text) in [(true, "縦向き"), (false, "横向き")] {
            ui.selectable_value(&mut layout.2, portrait, text);
        }
    });

    ui.horizontal_wrapped(|ui| {
        for &(c, r) in FIXED {
            let selected = !is_custom && (layout.0, layout.1) == (c, r);
            if tile(ui, (c, r, layout.2), selected, &format!("横{c}×縦{r}")).clicked() {
                (layout.0, layout.1) = (c, r);
                ui.data_mut(|d| d.insert_temp(custom_id, false));
            }
        }
        // 分割数を自由に決めるので、線は描かず「?」を置く
        if tile(ui, (1, 1, layout.2), is_custom, "任意").clicked() {
            ui.data_mut(|d| d.insert_temp(custom_id, true));
        }
    });

    if is_custom {
        ui.horizontal(|ui| {
            ui.label("分割数");
            for (value, prefix) in [(&mut layout.0, "横 "), (&mut layout.1, "縦 ")] {
                ui.add(
                    egui::DragValue::new(value)
                        .range(1..=CUSTOM_MAX)
                        // 範囲外の値を持つ既存テンプレートを、開いただけで書き換えない
                        .clamp_existing_to_range(false)
                        .prefix(prefix),
                );
            }
        });
    }
    ui.label(layout_label(layout.0, layout.1, layout.2));
    *layout != before
}

/// 用紙を分割線つきで描いた 1 枚のタイル
fn tile(
    ui: &mut Ui,
    (cols, rows, portrait): Layout,
    selected: bool,
    caption: &str,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(TILE, Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::RadioButton, true, selected, caption)
    });
    let visuals = ui.style().interact_selectable(&response, selected);
    let painter = ui.painter();
    painter.rect(
        rect,
        4.0,
        if selected {
            ui.visuals().selection.bg_fill.gamma_multiply(0.35)
        } else {
            visuals.weak_bg_fill
        },
        visuals.bg_stroke,
        StrokeKind::Inside,
    );

    let short = SHEET_LONG * (A4_SHORT_INCH / A4_LONG_INCH) as f32;
    let size = if portrait {
        vec2(short, SHEET_LONG)
    } else {
        vec2(SHEET_LONG, short)
    };
    let sheet = Rect::from_center_size(rect.center_top() + vec2(0.0, 4.0 + SHEET_LONG / 2.0), size);
    painter.rect(
        sheet,
        0.0,
        Color32::WHITE,
        Stroke::new(1.0, Color32::from_gray(0x70)),
        StrokeKind::Inside,
    );
    if caption == "任意" {
        painter.text(
            sheet.center(),
            egui::Align2::CENTER_CENTER,
            "?",
            egui::FontId::proportional(20.0),
            Color32::from_gray(0x70),
        );
    }
    let line = Stroke::new(1.0, Color32::from_rgb(0x2f, 0x6f, 0xb5));
    for i in 1..cols.max(1) {
        let x = sheet.left() + sheet.width() * f32::from(i) / f32::from(cols);
        painter.line_segment([pos2(x, sheet.top()), pos2(x, sheet.bottom())], line);
    }
    for j in 1..rows.max(1) {
        let y = sheet.top() + sheet.height() * f32::from(j) / f32::from(rows);
        painter.line_segment([pos2(sheet.left(), y), pos2(sheet.right(), y)], line);
    }
    painter.text(
        rect.center_bottom() - vec2(0.0, 4.0),
        egui::Align2::CENTER_BOTTOM,
        caption,
        egui::FontId::proportional(11.0),
        ui.visuals().text_color(),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    fn harness(layout: Layout) -> Harness<'static, Layout> {
        Harness::builder()
            .with_size(vec2(300.0, 400.0))
            .allow_missing_glyphs()
            .with_accessibility_check(true)
            .build_ui_state(
                |ui, layout: &mut Layout| {
                    show(ui, "t", layout);
                },
                layout,
            )
    }

    #[test]
    fn clicking_a_tile_sets_the_division_and_keeps_orientation() {
        let cases = [
            ((1u16, 3u16, true), "横2×縦3"),
            ((1, 1, false), "横4×縦5"),
            ((4, 5, true), "横1×縦1"),
        ];
        for (start, tile) in cases {
            let mut h = harness(start);
            h.get_by_label(tile).click();
            h.run();
            let want: (u16, u16) = tile
                .trim_start_matches('横')
                .split_once("×縦")
                .map(|(c, r)| (c.parse().unwrap(), r.parse().unwrap()))
                .unwrap();
            assert_eq!(*h.state(), (want.0, want.1, start.2), "{tile}");
        }
    }

    #[test]
    fn orientation_toggle_keeps_the_division() {
        let mut h = harness((2, 3, true));
        h.get_by_label("横向き").click();
        h.run();
        assert_eq!(*h.state(), (2, 3, false));
        h.get_by_label("縦向き").click();
        h.run();
        assert_eq!(*h.state(), (2, 3, true));
    }

    #[test]
    fn fixed_division_marks_its_tile_and_hides_the_free_inputs() {
        let h = harness((3, 4, true));
        assert!(h.query_by_label("分割数").is_none());
        let selected = |h: &Harness<'_, Layout>, name: &str| {
            use egui_kittest::kittest::NodeT;
            h.get_by_label(name).accesskit_node().toggled().is_some()
        };
        assert!(selected(&h, "横3×縦4"));
    }

    #[test]
    fn division_outside_the_fixed_list_opens_the_free_inputs() {
        let mut h = harness((5, 5, true));
        h.get_by_label("横");
        h.get_by_label("縦");
        // 範囲外の既存の値は、開いただけでは書き換えない
        assert_eq!(*h.state(), (5, 5, true));
        h.run();
        assert_eq!(*h.state(), (5, 5, true));
    }

    #[test]
    fn free_option_opens_the_inputs_and_edits_the_division() {
        let mut h = harness((1, 3, true));
        h.get_by_label("任意").click();
        h.run();
        assert_eq!(*h.state(), (1, 3, true), "開いただけでは変わらない");
        h.get_by_label("横").set_value_accesskit(6.0);
        h.run();
        h.get_by_label("縦").set_value_accesskit(7.0);
        h.run();
        assert_eq!(*h.state(), (6, 7, true));
        // 固定の候補を選ぶと、入力欄は閉じる
        h.get_by_label("横1×縦2").click();
        h.run();
        assert_eq!(*h.state(), (1, 2, true));
        assert!(h.query_by_label("分割数").is_none());
    }

    #[test]
    fn out_of_range_existing_value_is_not_rewritten() {
        let mut h = harness((12, 1, true));
        h.run();
        assert_eq!(*h.state(), (12, 1, true));
    }
}
