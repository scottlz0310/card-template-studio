//! 色の選択（代表的な色のパレット）。Excel の色選択に倣い、テーマの色（明るさの段つき）・標準の色・
//! 最近使った色・その他の色（標準のカラーピッカー）を、1 つのポップアップで選べる。
//!
//! 色は RGB（`[u8; 3]`）で扱う。TPL の COLORREF（BGR）との変換は、呼び出し側が持つ既存の関数で行う。
//! 最近使った色は、アプリの設定（`settings.json`）に保存する。ここでは egui の記憶（`Context::data`）に持ち、
//! アプリ本体が、起動時に読み込み、変わったら設定へ書き出す（`recent` / `set_recent`）。

use egui::color_picker::{Alpha, color_picker_color32};
use egui::{
    Color32, Context, CornerRadius, Id, Popup, PopupCloseBehavior, RichText, Role, Sense, Stroke,
    StrokeKind, Ui, WidgetInfo, vec2,
};

pub type Rgb = [u8; 3];

/// 最近使った色の最大数
pub const RECENT_MAX: usize = 10;

/// 色見本 1 つの大きさ（px）
const SWATCH: f32 = 20.0;

/// テーマの色の基本色（Office の既定のテーマ。左から 白・黒・薄い灰色・濃い青・アクセント 1〜6）
const THEME_BASE: [Rgb; 10] = [
    [0xff, 0xff, 0xff],
    [0x00, 0x00, 0x00],
    [0xe7, 0xe6, 0xe6],
    [0x44, 0x54, 0x6a],
    [0x44, 0x72, 0xc4],
    [0xed, 0x7d, 0x31],
    [0xa5, 0xa5, 0xa5],
    [0xff, 0xc0, 0x00],
    [0x5b, 0x9b, 0xd5],
    [0x70, 0xad, 0x47],
];

/// 標準の色（暗い赤・赤・オレンジ・黄・薄い緑・緑・水色・青・濃い青・紫）
const STANDARD: [Rgb; 10] = [
    [0xc0, 0x00, 0x00],
    [0xff, 0x00, 0x00],
    [0xff, 0xc0, 0x00],
    [0xff, 0xff, 0x00],
    [0x92, 0xd0, 0x50],
    [0x00, 0xb0, 0x50],
    [0x00, 0xb0, 0xf0],
    [0x00, 0x70, 0xc0],
    [0x00, 0x20, 0x60],
    [0x70, 0x30, 0xa0],
];

/// 白・黒・薄い灰色の列は、Excel と同じ、決め打ちの灰色の段（明るさの計算では、同じ段にならない）
const FIXED_COLUMNS: [(usize, [Rgb; 6]); 3] = [
    (
        0,
        [
            [0xff, 0xff, 0xff],
            [0xf2, 0xf2, 0xf2],
            [0xd9, 0xd9, 0xd9],
            [0xbf, 0xbf, 0xbf],
            [0xa6, 0xa6, 0xa6],
            [0x7f, 0x7f, 0x7f],
        ],
    ),
    (
        1,
        [
            [0x00, 0x00, 0x00],
            [0x7f, 0x7f, 0x7f],
            [0x59, 0x59, 0x59],
            [0x3f, 0x3f, 0x3f],
            [0x26, 0x26, 0x26],
            [0x0c, 0x0c, 0x0c],
        ],
    ),
    (
        2,
        [
            [0xe7, 0xe6, 0xe6],
            [0xd0, 0xce, 0xce],
            [0xae, 0xab, 0xab],
            [0x75, 0x71, 0x71],
            [0x3a, 0x38, 0x38],
            [0x17, 0x16, 0x16],
        ],
    ),
];

/// テーマの色の列（上から 基本色・明るく 80% / 60% / 40%・暗く 25% / 50%）。`i` は 0〜9
pub fn theme_column(i: usize) -> [Rgb; 6] {
    if let Some((_, col)) = FIXED_COLUMNS.iter().find(|(k, _)| *k == i) {
        return *col;
    }
    let base = THEME_BASE[i];
    // Office の「明るさの変更」: 輝度（HSL の L）に、掛ける値と足す値を当てる
    [
        base,
        tint(base, 0.2, 0.8),
        tint(base, 0.4, 0.6),
        tint(base, 0.6, 0.4),
        tint(base, 0.75, 0.0),
        tint(base, 0.5, 0.0),
    ]
}

/// 輝度（HSL の L）に `l × lum_mod + lum_off` を当てた色
fn tint(rgb: Rgb, lum_mod: f64, lum_off: f64) -> Rgb {
    let (h, s, l) = to_hsl(rgb);
    from_hsl(h, s, (l * lum_mod + lum_off).clamp(0.0, 1.0))
}

fn to_hsl(rgb: Rgb) -> (f64, f64, f64) {
    let [r, g, b] = rgb.map(|c| f64::from(c) / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    let d = max - min;
    if d == 0.0 {
        return (0.0, 0.0, l);
    }
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

fn from_hsl(h: f64, s: f64, l: f64) -> Rgb {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h / 60.0).floor() as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r, g, b].map(|v| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8)
}

/// `#RRGGBB`
pub fn hex(rgb: Rgb) -> String {
    format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2])
}

/// 最近使った色の並びに `c` を足す。すでにあれば先頭へ移し、最大 `RECENT_MAX` 個に保つ
pub fn pushed(list: &[Rgb], c: Rgb) -> Vec<Rgb> {
    let mut out: Vec<Rgb> = std::iter::once(c)
        .chain(list.iter().copied().filter(|&x| x != c))
        .collect();
    out.truncate(RECENT_MAX);
    out
}

const RECENT_ID: &str = "cts-recent-colors";

/// 最近使った色（新しい順）
pub fn recent(ctx: &Context) -> Vec<Rgb> {
    ctx.data(|d| d.get_temp::<Vec<Rgb>>(Id::unique(RECENT_ID)))
        .unwrap_or_default()
}

/// 最近使った色を置き換える（起動時に、保存してある設定から読み込む）
pub fn set_recent(ctx: &Context, list: &[Rgb]) {
    let mut list = list.to_vec();
    list.truncate(RECENT_MAX);
    ctx.data_mut(|d| d.insert_temp(Id::unique(RECENT_ID), list));
}

fn push_recent(ctx: &Context, c: Rgb) {
    set_recent(ctx, &pushed(&recent(ctx), c));
}

/// 色のボタン。押すとパレットが開く。色が変わったとき `true`
pub fn color_button(ui: &mut Ui, rgb: &mut Rgb) -> bool {
    let popup_id = ui.auto_id_with("palette-popup");
    let picked_id = popup_id.with("picked");
    let was_open = Popup::is_id_open(ui.ctx(), popup_id);
    let (rect, response) = ui.allocate_exact_size(vec2(40.0, 18.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact_selectable(&response, was_open);
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(2),
            Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
        );
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(2),
            Stroke::new(1.0, visuals.bg_stroke.color),
            StrokeKind::Inside,
        );
    }
    let name = format!("色 {}", hex(*rgb));
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, name.clone()));
    let mut response = response.on_hover_text(hex(*rgb));
    let mut changed = false;
    Popup::menu(&response)
        .id(popup_id)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            changed = palette_contents(ui, rgb, popup_id, picked_id);
        });
    // 「その他の色」で変えた色は、ドラッグの途中で何度も変わるので、閉じたときに、最近使った色へ入れる
    if was_open
        && !Popup::is_id_open(ui.ctx(), popup_id)
        && ui
            .data_mut(|d| d.remove_temp::<bool>(picked_id))
            .unwrap_or(false)
    {
        push_recent(ui.ctx(), *rgb);
    }
    if changed {
        response.mark_changed();
    }
    changed
}

/// ポップアップの中身。色が変わったとき `true`
fn palette_contents(ui: &mut Ui, rgb: &mut Rgb, popup_id: Id, picked_id: Id) -> bool {
    let mut changed = false;
    ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
    ui.label(RichText::new("テーマの色").small());
    let columns: Vec<[Rgb; 6]> = (0..THEME_BASE.len()).map(theme_column).collect();
    for row in 0..6 {
        ui.horizontal(|ui| {
            for col in &columns {
                changed |= swatch(ui, col[row], rgb);
            }
        });
    }
    ui.add_space(4.0);
    ui.label(RichText::new("標準の色").small());
    ui.horizontal(|ui| {
        for c in STANDARD {
            changed |= swatch(ui, c, rgb);
        }
    });
    let recent = recent(ui.ctx());
    if !recent.is_empty() {
        ui.add_space(4.0);
        ui.label(RichText::new("最近使った色").small());
        ui.horizontal(|ui| {
            for c in recent {
                changed |= swatch(ui, c, rgb);
            }
        });
    }
    ui.separator();
    let more_id = popup_id.with("more");
    let mut more = ui.data(|d| d.get_temp::<bool>(more_id)).unwrap_or(false);
    if ui.selectable_label(more, "その他の色…").clicked() {
        more = !more;
        ui.data_mut(|d| d.insert_temp(more_id, more));
    }
    if more {
        let mut c = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
        if color_picker_color32(ui, &mut c, Alpha::Opaque) {
            *rgb = [c.r(), c.g(), c.b()];
            changed = true;
            ui.data_mut(|d| d.insert_temp(picked_id, true));
        }
    }
    changed
}

/// 色見本 1 つ。押すと `rgb` をその色にして、最近使った色へ入れ、ポップアップを閉じる
fn swatch(ui: &mut Ui, color: Rgb, rgb: &mut Rgb) -> bool {
    let (rect, response) = ui.allocate_exact_size(vec2(SWATCH, SWATCH), Sense::click());
    let name = format!("色 {}", hex(color));
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, name.clone()));
    let current = *rgb == color;
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(1),
            Color32::from_rgb(color[0], color[1], color[2]),
        );
        let stroke = if current {
            ui.visuals().selection.stroke
        } else if response.hovered() {
            ui.visuals().widgets.hovered.fg_stroke
        } else {
            Stroke::new(1.0, Color32::from_gray(160))
        };
        ui.painter()
            .rect_stroke(rect, CornerRadius::same(1), stroke, StrokeKind::Inside);
    }
    let response = response.on_hover_text(hex(color));
    if response.clicked() {
        *rgb = color;
        push_recent(ui.ctx(), color);
        ui.close();
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Rgb, b: Rgb) -> bool {
        a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 1)
    }

    #[test]
    fn theme_columns_match_the_office_default_palette() {
        // (基本色の列, Excel の「明るく 80% / 60% / 40%、暗く 25% / 50%」の値)
        let cases: [(usize, [Rgb; 5]); 7] = [
            (
                3,
                [
                    [0xd6, 0xdc, 0xe5],
                    [0xac, 0xb9, 0xca],
                    [0x84, 0x97, 0xb0],
                    [0x33, 0x3f, 0x50],
                    [0x22, 0x2a, 0x35],
                ],
            ),
            (
                4,
                [
                    [0xd9, 0xe2, 0xf3],
                    [0xb4, 0xc7, 0xe7],
                    [0x8f, 0xaa, 0xdc],
                    [0x2f, 0x55, 0x97],
                    [0x1f, 0x38, 0x64],
                ],
            ),
            (
                5,
                [
                    [0xfb, 0xe5, 0xd6],
                    [0xf8, 0xcb, 0xad],
                    [0xf4, 0xb1, 0x83],
                    [0xc5, 0x5a, 0x11],
                    [0x84, 0x3c, 0x0c],
                ],
            ),
            (
                6,
                [
                    [0xed, 0xed, 0xed],
                    [0xdb, 0xdb, 0xdb],
                    [0xc9, 0xc9, 0xc9],
                    [0x7c, 0x7c, 0x7c],
                    [0x52, 0x52, 0x52],
                ],
            ),
            (
                7,
                [
                    [0xff, 0xf2, 0xcc],
                    [0xff, 0xe6, 0x99],
                    [0xff, 0xd9, 0x66],
                    [0xbf, 0x90, 0x00],
                    [0x7f, 0x60, 0x00],
                ],
            ),
            (
                8,
                [
                    [0xde, 0xeb, 0xf7],
                    [0xbd, 0xd7, 0xee],
                    [0x9d, 0xc3, 0xe6],
                    [0x2e, 0x75, 0xb6],
                    [0x1f, 0x4e, 0x79],
                ],
            ),
            (
                9,
                [
                    [0xe2, 0xf0, 0xd9],
                    [0xc5, 0xe0, 0xb4],
                    [0xa9, 0xd1, 0x8e],
                    [0x54, 0x82, 0x35],
                    [0x38, 0x57, 0x23],
                ],
            ),
        ];
        for (i, want) in cases {
            let col = theme_column(i);
            assert_eq!(col[0], THEME_BASE[i], "列 {i} の基本色");
            for (k, w) in want.iter().enumerate() {
                assert!(
                    close(col[k + 1], *w),
                    "列 {i} の段 {}: {} != {}",
                    k + 1,
                    hex(col[k + 1]),
                    hex(*w)
                );
            }
        }
    }

    #[test]
    fn white_black_and_gray_columns_use_the_fixed_ramps() {
        for i in [0, 1, 2] {
            let col = theme_column(i);
            assert_eq!(col[0], THEME_BASE[i], "列 {i} の基本色");
        }
        assert_eq!(theme_column(0)[5], [0x7f, 0x7f, 0x7f]);
        assert_eq!(theme_column(1)[1], [0x7f, 0x7f, 0x7f]);
        assert_eq!(theme_column(2)[1], [0xd0, 0xce, 0xce]);
    }

    #[test]
    fn hsl_round_trips_every_theme_and_standard_color() {
        for c in THEME_BASE.into_iter().chain(STANDARD) {
            let (h, s, l) = to_hsl(c);
            assert!(close(from_hsl(h, s, l), c), "{}", hex(c));
        }
    }

    #[test]
    fn recent_colors_move_to_the_front_without_duplicates_and_stay_within_the_limit() {
        // (名前, 今の並び, 足す色, 結果)
        type Case = (&'static str, Vec<Rgb>, Rgb, Vec<Rgb>);
        let (a, b, c) = ([1, 1, 1], [2, 2, 2], [3, 3, 3]);
        let many: Vec<Rgb> = (0..RECENT_MAX as u8).map(|i| [i, 0, 0]).collect();
        let mut expected_many = vec![[200, 0, 0]];
        expected_many.extend(many.iter().take(RECENT_MAX - 1));
        let cases: Vec<Case> = vec![
            ("空のとき", vec![], a, vec![a]),
            ("新しい色は先頭に", vec![a, b], c, vec![c, a, b]),
            (
                "すでにある色は、重複せず先頭へ",
                vec![a, b, c],
                c,
                vec![c, a, b],
            ),
            ("先頭の色を足しても変わらない", vec![a, b], a, vec![a, b]),
            (
                "最大数を超えたら、いちばん古い色が消える",
                many,
                [200, 0, 0],
                expected_many,
            ),
        ];
        for (name, list, add, want) in cases {
            assert_eq!(pushed(&list, add), want, "{name}");
        }
    }

    #[test]
    fn hex_is_upper_case_rrggbb() {
        assert_eq!(hex([0x4c, 0xa3, 0x0a]), "#4CA30A");
    }
}
