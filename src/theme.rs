//! 画面のテーマ（システムに合わせる／ライト／ダーク）と、テーマごとの色。
//!
//! カードの用紙（白い面）と、その上に描くもの（見本の絵・図形・ロックの表示など）は、印刷物のプレビュー
//! なので、どのテーマでも、いまの色のまま（白い用紙に、既定の黒い文字）。テーマで変えるのは、用紙の
//! 外側（作業面・ルーラー）と、画面の部品（パネル・ダイアログ・文字）の色。

use egui::{Color32, Context, Theme, ThemePreference, Ui};
use serde::{Deserialize, Serialize};

/// 利用者が選ぶテーマ。既定は、システムに合わせる
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeChoice {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "システムに合わせる",
            Self::Light => "ライト",
            Self::Dark => "ダーク",
        }
    }

    /// egui の設定。システムに合わせるときは、OS のテーマの変更に、実行中も追従する
    pub fn preference(self) -> ThemePreference {
        match self {
            Self::System => ThemePreference::System,
            Self::Light => ThemePreference::Light,
            Self::Dark => ThemePreference::Dark,
        }
    }
}

/// テーマごとに決める色（用紙の上に描くものは、含めない）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Colors {
    /// キャンバスの、用紙の外側（作業面）
    pub workspace: Color32,
    pub ruler_bg: Color32,
    pub ruler_fg: Color32,
    pub ruler_line: Color32,
    /// 左右のパネルの背景
    pub panel: Color32,
    /// 警告・エラーの文字
    pub error: Color32,
    /// 補足（できた・使える）の文字
    pub ok: Color32,
    /// 見出しなどの、アクセント色の文字
    pub accent_text: Color32,
}

impl Colors {
    pub fn of(dark: bool) -> Self {
        if dark {
            Self {
                workspace: Color32::from_rgb(0x2b, 0x2f, 0x36),
                ruler_bg: Color32::from_rgb(0x22, 0x26, 0x2c),
                ruler_fg: Color32::from_rgb(0x9a, 0xa3, 0xae),
                ruler_line: Color32::from_gray(70),
                panel: Color32::from_rgb(0x22, 0x25, 0x2a),
                error: Color32::from_rgb(0xff, 0x7b, 0x6b),
                ok: Color32::from_rgb(0x5f, 0xd3, 0x9a),
                accent_text: Color32::from_rgb(0x6e, 0xa8, 0xfe),
            }
        } else {
            Self {
                workspace: Color32::from_rgb(0xdf, 0xe3, 0xe8),
                ruler_bg: Color32::from_rgb(0xf3, 0xf5, 0xf8),
                ruler_fg: Color32::from_rgb(0x63, 0x6c, 0x78),
                ruler_line: Color32::from_gray(200),
                panel: Color32::from_rgb(0xf7, 0xf8, 0xfa),
                error: Color32::from_rgb(0xc0, 0x39, 0x2b),
                ok: Color32::from_rgb(0x2e, 0x7d, 0x57),
                // 選択の塗り（ACCENT = #1F6FEB）より、わずかに濃くする（パネルの背景に対して、コントラスト比 4.5 以上にするため）
                accent_text: Color32::from_rgb(0x1a, 0x68, 0xde),
            }
        }
    }

    /// いま描いている画面のテーマの色
    pub fn of_ui(ui: &Ui) -> Self {
        Self::of(ui.visuals().dark_mode)
    }
}

/// 選んだテーマを、画面に反映する
pub fn apply_choice(ctx: &Context, choice: ThemeChoice) {
    ctx.set_theme(choice.preference());
}

/// ライト・ダークの両方に、このアプリの見た目（余白・角丸・選択の色・パネルの背景）を当てる
pub fn apply_style(ctx: &Context, accent: Color32) {
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
        s.spacing.button_padding = egui::vec2(8.0, 4.0);
        s.visuals.selection.bg_fill = accent;
        s.visuals.selection.stroke = egui::Stroke::new(1.0, Color32::WHITE);
        s.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(5);
        s.visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(5);
        s.visuals.widgets.active.corner_radius = egui::CornerRadius::same(5);
    });
    ctx.style_mut_of(Theme::Light, |s| {
        s.visuals.panel_fill = Colors::of(false).panel;
    });
    ctx.style_mut_of(Theme::Dark, |s| {
        s.visuals.panel_fill = Colors::of(true).panel;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG の相対輝度とコントラスト比
    fn luminance(c: Color32) -> f64 {
        let f = |v: u8| {
            let s = f64::from(v) / 255.0;
            if s <= 0.03928 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    #[test]
    fn texts_are_readable_on_the_panel_in_both_themes() {
        // 文字の色は、パネルの背景に対して、コントラスト比 4.5 以上（WCAG の通常の文字）
        for dark in [false, true] {
            let c = Colors::of(dark);
            for (name, fg) in [
                ("警告・エラー", c.error),
                ("補足", c.ok),
                ("見出し", c.accent_text),
                ("ルーラーの目盛り", c.ruler_fg),
            ] {
                let bg = if name == "ルーラーの目盛り" {
                    c.ruler_bg
                } else {
                    c.panel
                };
                let ratio = contrast(fg, bg);
                assert!(ratio >= 4.5, "dark={dark} {name}: {ratio:.2}");
            }
        }
    }

    #[test]
    fn dark_theme_is_darker_than_the_light_theme_where_it_should_be() {
        let (l, d) = (Colors::of(false), Colors::of(true));
        for (name, light, dark) in [
            ("作業面", l.workspace, d.workspace),
            ("ルーラー", l.ruler_bg, d.ruler_bg),
            ("パネル", l.panel, d.panel),
        ] {
            assert!(luminance(dark) < luminance(light) * 0.5, "{name}");
        }
    }

    #[test]
    fn choice_serializes_as_lowercase_and_defaults_to_system() {
        assert_eq!(ThemeChoice::default(), ThemeChoice::System);
        for (choice, json) in [
            (ThemeChoice::System, "\"system\""),
            (ThemeChoice::Light, "\"light\""),
            (ThemeChoice::Dark, "\"dark\""),
        ] {
            assert_eq!(serde_json::to_string(&choice).unwrap(), json);
            assert_eq!(serde_json::from_str::<ThemeChoice>(json).unwrap(), choice);
        }
        assert!(serde_json::from_str::<ThemeChoice>("\"neon\"").is_err());
    }

    #[test]
    fn choices_map_to_the_egui_preference() {
        for (choice, pref) in [
            (ThemeChoice::System, ThemePreference::System),
            (ThemeChoice::Light, ThemePreference::Light),
            (ThemeChoice::Dark, ThemePreference::Dark),
        ] {
            assert_eq!(choice.preference(), pref);
        }
    }
}
