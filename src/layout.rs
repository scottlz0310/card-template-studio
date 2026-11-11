//! カードの大きさと配置範囲。
//!
//! カードの大きさは A4 ÷ 分割数。既存テンプレートの座標（OBJ の矩形）を実測した値は、
//! 「これまで実際に使われていた配置範囲」の目安として画面に点線で添えるだけに使う。

use crate::tpl::{A4_LONG_INCH, A4_SHORT_INCH, Header, Template};

/// 既存サンプルの OBJ 矩形の実測（インチ）。最小の左・上、最大の右・下
#[derive(Clone, Copy, Debug)]
pub struct LayoutRef {
    pub cols: u16,
    pub rows: u16,
    pub portrait: bool,
    pub min_left: f64,
    pub min_top: f64,
    pub max_right: f64,
    pub max_bottom: f64,
    pub samples: u32,
}

/// 2026-11-09 に TMPEXP の 70 テンプレートから測った値（mechanics.json と同じ）
pub const MEASURED: &[LayoutRef] = &[
    LayoutRef {
        cols: 1,
        rows: 1,
        portrait: false,
        min_left: 0.1,
        min_top: 2.029,
        max_right: 11.585,
        max_bottom: 7.902,
        samples: 2,
    },
    LayoutRef {
        cols: 1,
        rows: 2,
        portrait: true,
        min_left: 0.1,
        min_top: 0.17,
        max_right: 8.168,
        max_bottom: 5.681,
        samples: 2,
    },
    LayoutRef {
        cols: 1,
        rows: 3,
        portrait: true,
        min_left: 0.1,
        min_top: 0.1,
        max_right: 8.268,
        max_bottom: 3.898,
        samples: 56,
    },
    LayoutRef {
        cols: 1,
        rows: 4,
        portrait: true,
        min_left: 0.384,
        min_top: 0.1,
        max_right: 8.168,
        max_bottom: 2.672,
        samples: 1,
    },
    LayoutRef {
        cols: 2,
        rows: 1,
        portrait: false,
        min_left: 0.117,
        min_top: 1.763,
        max_right: 5.763,
        max_bottom: 7.968,
        samples: 2,
    },
    LayoutRef {
        cols: 2,
        rows: 2,
        portrait: false,
        min_left: 0.1,
        min_top: 0.691,
        max_right: 5.746,
        max_bottom: 3.946,
        samples: 4,
    },
    LayoutRef {
        cols: 4,
        rows: 5,
        portrait: true,
        min_left: 0.1,
        min_top: 0.1,
        max_right: 1.967,
        max_bottom: 2.223,
        samples: 2,
    },
    LayoutRef {
        cols: 5,
        rows: 5,
        portrait: true,
        min_left: 0.1,
        min_top: 0.1,
        max_right: 1.554,
        max_bottom: 1.916,
        samples: 1,
    },
];

pub fn measured(h: &Header) -> Option<&'static LayoutRef> {
    MEASURED
        .iter()
        .find(|m| m.cols == h.cols && m.rows == h.rows && m.portrait == h.is_portrait())
}

/// 画面に描くカードの枠
#[derive(Clone, Copy, Debug)]
pub struct CardFrame {
    /// カードの幅・高さ（インチ）
    pub width: f64,
    pub height: f64,
    /// 既存テンプレートで使われている配置範囲（インチ, left/top/right/bottom）
    pub used: Option<[f64; 4]>,
}

/// カード枠を決める。カードの大きさは、A4 ÷ 分割数（実機で 1×4 縦の緑の枠を測り、幅と高さの比が
/// A4 ÷ 分割数に 0.1% 以内で一致した）。開いているテンプレートの物がはみ出すときは広げる。
/// 既存テンプレートで使われていた配置範囲（実測）が分かるレイアウトは、`used` に添える。
pub fn card_frame(t: &Template) -> CardFrame {
    let (mut w, mut h) = a4_division(&t.header);
    for o in &t.objects {
        let [_, _, r, b] = o.normalized();
        w = w.max(r);
        h = h.max(b);
    }
    CardFrame {
        width: w,
        height: h,
        used: measured(&t.header).map(|m| [m.min_left, m.min_top, m.max_right, m.max_bottom]),
    }
}

pub fn a4_division(h: &Header) -> (f64, f64) {
    let (pw, ph) = if h.is_portrait() {
        (A4_SHORT_INCH, A4_LONG_INCH)
    } else {
        (A4_LONG_INCH, A4_SHORT_INCH)
    };
    (pw / h.cols.max(1) as f64, ph / h.rows.max(1) as f64)
}

pub fn layout_label(cols: u16, rows: u16, portrait: bool) -> String {
    format!(
        "横{cols}×縦{rows}（{}）",
        if portrait { "縦向き" } else { "横向き" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tpl::{Body, CaptionBody, Object};

    #[test]
    fn frame_is_the_a4_division_for_every_layout() {
        // 実機で測ると、実測の材料（既存テンプレートの座標）が偏っているレイアウトでも A4 ÷ 分割数だった
        let cases = [
            (1u16, 3u16, true),
            (1, 4, true),
            (1, 1, false),
            (2, 1, false),
            (2, 2, false),
            (5, 5, true),
            (3, 4, true),
        ];
        for (cols, rows, portrait) in cases {
            let t = Template {
                header: header(cols, rows, portrait),
                ..Template::default()
            };
            let f = card_frame(&t);
            let (w, h) = a4_division(&t.header);
            assert!(
                (f.width - w).abs() < 1e-9 && (f.height - h).abs() < 1e-9,
                "{cols}x{rows} {portrait}"
            );
        }
    }

    #[test]
    fn frame_grows_to_fit_objects() {
        let mut t = Template::default();
        t.objects.push(Object::new(
            Body::Caption(CaptionBody::new("a", "", false, 12)),
            [0.0, 0.0, 20.0, 9.0],
        ));
        let f = card_frame(&t);
        assert!(f.width >= 20.0 && f.height >= 9.0);
    }

    fn header(cols: u16, rows: u16, portrait: bool) -> Header {
        Header {
            cols,
            rows,
            orientation: portrait as u32,
            ..Header::default()
        }
    }

    #[test]
    fn measured_lookup_matches_layout_and_orientation() {
        let cases = [
            (1u16, 3u16, true, Some(56u32)),
            (1, 3, false, None),
            (2, 2, false, Some(4)),
            (2, 2, true, None),
            (9, 9, true, None),
        ];
        for (cols, rows, portrait, samples) in cases {
            let m = measured(&header(cols, rows, portrait));
            assert_eq!(m.map(|m| m.samples), samples, "{cols}x{rows} {portrait}");
        }
    }

    #[test]
    fn used_range_is_attached_only_where_measured() {
        let cases = [((1u16, 3u16), true), ((3, 4), false)];
        for ((cols, rows), has) in cases {
            let t = Template {
                header: header(cols, rows, true),
                ..Template::default()
            };
            let f = card_frame(&t);
            let m = measured(&t.header);
            assert_eq!(f.used.is_some(), has, "{cols}x{rows}");
            assert_eq!(
                f.used,
                m.map(|m| [m.min_left, m.min_top, m.max_right, m.max_bottom])
            );
        }
    }
    #[test]
    fn a4_division_splits_the_sheet() {
        let (w, h) = a4_division(&header(2, 3, true));
        assert!((w - A4_SHORT_INCH / 2.0).abs() < 1e-9);
        assert!((h - A4_LONG_INCH / 3.0).abs() < 1e-9);
        let (w, h) = a4_division(&header(2, 3, false));
        assert!((w - A4_LONG_INCH / 2.0).abs() < 1e-9);
        assert!((h - A4_SHORT_INCH / 3.0).abs() < 1e-9);
        // 0 分割でも 0 除算にならない
        let (w, h) = a4_division(&header(0, 0, true));
        assert!(w.is_finite() && h.is_finite());
    }

    #[test]
    fn frame_normalizes_reversed_rects() {
        let mut t = Template::default();
        t.objects.push(Object::new(
            Body::Caption(CaptionBody::new("a", "", false, 12)),
            [30.0, 25.0, 0.0, 0.0],
        ));
        let f = card_frame(&t);
        assert!(f.width >= 30.0 && f.height >= 25.0);
    }

    #[test]
    fn layout_label_names_division_and_orientation() {
        assert_eq!(layout_label(1, 3, true), "横1×縦3（縦向き）");
        assert_eq!(layout_label(2, 1, false), "横2×縦1（横向き）");
    }

    #[test]
    fn measured_values_are_sane() {
        for m in MEASURED {
            assert!(m.min_left < m.max_right && m.min_top < m.max_bottom);
            assert!(m.samples > 0);
        }
    }
}
