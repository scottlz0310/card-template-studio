use super::error::ParseError;
use super::io::{Reader, Writer};
use super::*;

fn sample() -> Template {
    let mut t = Template::default();
    t.objects.push(Object::new(
        Body::Picture(PictureBody::default()),
        [0.1, 0.25, 4.63, 3.65],
    ));
    t.objects.push(Object::new(
        Body::Caption(CaptionBody::new("写真タイトル", "写真タイトル", false, 12)),
        [4.73, 0.25, 5.67, 0.45],
    ));
    t.objects.push(Object::new(
        Body::Caption(CaptionBody::new("", "写真タイトル", true, 12)),
        [5.67, 0.25, 8.17, 0.45],
    ));
    t
}

/// 図形（種類 7）の本体: 共通 45 バイト + u32 ×5 + double ×2
fn rect_shape_bytes(shape: u32) -> Vec<u8> {
    let mut w = Writer::default();
    for v in [0u32, 0] {
        w.u32(v);
    }
    w.u8(0);
    for v in [1u32, 6, 1, 1, 0x0000ff, 1, 0, 0x00ffcc, 0] {
        w.u32(v);
    }
    for v in [shape, 0, 1, 8, 8] {
        w.u32(v);
    }
    w.f64(0.3);
    w.f64(0.3);
    w.out
}

#[test]
fn roundtrip_built_template() {
    let t = sample();
    let bytes = t.to_bytes();
    let back = Template::parse(&bytes).expect("parse");
    assert_eq!(back.objects, t.objects);
    assert_eq!(back.to_bytes(), bytes);
}

#[test]
fn passthrough_shapes_roundtrip_unchanged() {
    for shape in 0..5u32 {
        let mut t = sample();
        t.header.sub_version = 4;
        t.objects.push(Object {
            tag_version: 3,
            kind: KIND_RECT,
            link: 0,
            rect: [1.0, 1.0, 2.0, 2.0],
            body: Body::Passthrough(rect_shape_bytes(shape)),
            trailer: Some(7),
        });
        let bytes = t.to_bytes();
        let back = Template::parse(&bytes).expect("parse");
        assert_eq!(back.to_bytes(), bytes, "shape {shape}");
        let view = back.objects[3].shape_view().expect("view");
        assert_eq!(view.shape, ShapeKind::from_u32(shape));
        assert_eq!(view.base.pen_color, 0x0000ff);
        assert_eq!(back.objects[3].trailer, Some(7));
    }
}

#[test]
fn sub_version_follows_object_versions() {
    let cases: [(u8, u32, u16, u16); 4] = [
        (2, KIND_CAPTION, 2, 2),
        (1, KIND_CAPTION, 1, 1),
        (2, KIND_PICTURE, 2, 3),
        (3, KIND_RECT, 2, 4),
    ];
    for (tag_version, kind, sub, expected) in cases {
        let mut t = Template::default();
        t.header.sub_version = sub;
        let body = match kind {
            KIND_CAPTION => Body::Caption(CaptionBody::new("a", "", false, 12)),
            KIND_PICTURE => Body::Picture(PictureBody::default()),
            _ => Body::Passthrough(rect_shape_bytes(0)),
        };
        t.objects.push(Object {
            tag_version,
            kind,
            link: 0,
            rect: [0.0, 0.0, 1.0, 1.0],
            body,
            trailer: None,
        });
        assert_eq!(
            t.required_sub_version(),
            expected,
            "{kind} OBJ{tag_version}"
        );
    }
}

#[test]
fn folder_names() {
    let cases = [
        ("TEMP1x3_V", Some((1, 3, 1))),
        ("TEMP2x1_H", Some((2, 1, 0))),
        ("TEMPx_V", None),
        ("Other", None),
    ];
    for (name, expected) in cases {
        assert_eq!(parse_folder_name(name), expected, "{name}");
    }
    let h = Header {
        cols: 2,
        rows: 2,
        orientation: 0,
        ..Default::default()
    };
    assert_eq!(h.folder_name(), "TEMP2x2_H");
}

#[test]
fn rejects_bad_files() {
    let cases: [&[u8]; 3] = [b"", b"XXXX", b"TMP1\x01\x00\x09\x00"];
    for data in cases {
        assert!(Template::parse(data).is_err());
    }
}

/// 実物のテンプレートで「読んで書いたら同じバイト列」になるか。
/// CARD_TPL_SAMPLES に TMPEXP フォルダを指定したときだけ動く。
#[test]
fn roundtrip_real_samples() {
    let Ok(dir) = std::env::var("CARD_TPL_SAMPLES") else {
        return;
    };
    let mut n = 0;
    for sub in std::fs::read_dir(&dir).unwrap().flatten() {
        if !sub.path().is_dir() {
            continue;
        }
        for f in std::fs::read_dir(sub.path()).unwrap().flatten() {
            let p = f.path();
            if p.extension().map(|e| e.eq_ignore_ascii_case("tpl")) != Some(true) {
                continue;
            }
            let data = std::fs::read(&p).unwrap();
            let t = Template::parse(&data).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            assert_eq!(t.to_bytes(), data, "{}", p.display());
            n += 1;
        }
    }
    assert!(n > 0, "サンプルが見つかりません");
    eprintln!("{n} 個のテンプレートが一致");
}

// ----- 補助 -----

fn passthrough(kind: u32, body: Vec<u8>, tag_version: u8) -> Object {
    Object {
        tag_version,
        kind,
        link: 0,
        rect: [0.0, 0.0, 1.0, 1.0],
        body: Body::Passthrough(body),
        trailer: (tag_version >= 3).then_some(0),
    }
}

fn base_bytes(w: &mut Writer) {
    w.u32(0);
    w.u32(0);
    w.u8(0);
    for v in [1u32, 6, 1, 1, 0x0000ff, 1, 0, 0x00ffcc, 0] {
        w.u32(v);
    }
}

fn pattern_bytes() -> Vec<u8> {
    let mut b = rect_shape_bytes(2);
    b.extend_from_slice(&7u32.to_le_bytes());
    b
}

fn poly_bytes(points: &[(f64, f64)]) -> Vec<u8> {
    let mut w = Writer::default();
    base_bytes(&mut w);
    w.u32(points.len() as u32);
    for &(x, y) in points {
        w.f64(x);
        w.f64(y);
    }
    w.out
}

fn draw_object_bytes() -> Vec<u8> {
    (0..22).collect()
}

fn draft_bytes() -> Vec<u8> {
    let mut w = Writer::default();
    w.bytes(&[1; 40]);
    w.cstring(&SjisString::from_str("参考図"));
    w.cstring(&SjisString::from_str("a"));
    w.bytes(&[2; 16]);
    w.cstring(&SjisString::default());
    w.bytes(&[3; 4]);
    w.cstring(&SjisString::from_str("b"));
    w.bytes(&[4; 4]);
    w.out
}

fn cm_value(tag_version: u8) -> Object {
    let mut o = Object::new(
        Body::CmValue(CmValueBody {
            text: SjisString::from_str("施工管理値"),
            c0: 1,
            c4: 2,
            x138: 3,
            x13c: 4,
            x148: 5,
            face: string_to_fixed("ＭＳ ゴシック"),
            fmt: default_format(9),
            x140: 6,
            x154: 7,
            x150: 8,
            x14c: 9,
            x144: 10,
            ext: (tag_version > 1).then(|| (11, 12, vec![[5u8; 68], [6u8; 68]])),
        }),
        [0.0, 0.0, 2.0, 1.0],
    );
    o.tag_version = tag_version;
    o
}

fn everything() -> Template {
    let mut t = sample();
    t.header.sub_version = 4;
    t.card_color = 0x00c0_ffee;
    t.objects.push(cm_value(1));
    t.objects.push(cm_value(2));
    t.objects
        .push(passthrough(KIND_DRAW_OBJECT, draw_object_bytes(), 1));
    t.objects.push(passthrough(KIND_DRAFT, draft_bytes(), 2));
    t.objects
        .push(passthrough(KIND_RECT, rect_shape_bytes(1), 3));
    t.objects
        .push(passthrough(KIND_PATTERN, pattern_bytes(), 3));
    t.objects.push(passthrough(
        KIND_POLY,
        poly_bytes(&[(0.0, 0.0), (1.0, 0.5), (0.5, 1.0)]),
        3,
    ));
    t
}

// ----- 往復・種別 -----

#[test]
fn roundtrip_every_object_kind() {
    let t = everything();
    let bytes = t.to_bytes();
    let back = Template::parse(&bytes).expect("parse");
    assert_eq!(back.to_bytes(), bytes);
    assert_eq!(back.objects, t.objects);
    assert_eq!(back.card_color, 0x00c0_ffee);
    assert_eq!(back.header.sub_version, 4);
}

#[test]
fn old_tag_versions_drop_the_newer_fields() {
    // 版 1 の文字項目は枠線の色・太さを持たず、写真枠は +0xec を持たない
    let mut c = CaptionBody::new("a", "", false, 12);
    c.border_style = None;
    let mut t = Template::default();
    let mut o = Object::new(Body::Caption(c), [0.0, 0.0, 1.0, 1.0]);
    o.tag_version = 1;
    t.objects.push(o);
    t.objects.push(Object::new(
        Body::Picture(PictureBody::default()),
        [0.0, 1.0, 1.0, 2.0],
    ));
    let back = Template::parse(&t.to_bytes()).unwrap();
    assert_eq!(back.objects, t.objects);
    match &back.objects[0].body {
        Body::Caption(c) => assert!(c.border_style.is_none()),
        _ => unreachable!(),
    }
    match &back.objects[1].body {
        Body::Picture(p) => assert!(p.ec.is_none()),
        _ => unreachable!(),
    }
}

#[test]
fn picture_with_newer_tag_keeps_extra_field() {
    let p = PictureBody {
        ec: Some(0x1234),
        ..PictureBody::default()
    };
    let mut o = Object::new(Body::Picture(p), [0.0, 0.0, 1.0, 1.0]);
    o.tag_version = 2;
    let mut t = Template::default();
    t.objects.push(o);
    let back = Template::parse(&t.to_bytes()).unwrap();
    match &back.objects[0].body {
        Body::Picture(p) => assert_eq!(p.ec, Some(0x1234)),
        _ => unreachable!(),
    }
}

#[test]
fn reversed_rect_is_kept_but_normalized_on_demand() {
    let o = Object::new(
        Body::Caption(CaptionBody::new("a", "", false, 12)),
        [3.0, 4.0, 1.0, 2.0],
    );
    assert_eq!(o.rect, [3.0, 4.0, 1.0, 2.0]);
    assert_eq!(o.normalized(), [1.0, 2.0, 3.0, 4.0]);
    let mut t = Template::default();
    t.objects.push(o);
    let back = Template::parse(&t.to_bytes()).unwrap();
    assert_eq!(back.objects[0].rect, [3.0, 4.0, 1.0, 2.0]);
}

#[test]
fn long_strings_use_extended_length() {
    let cases = [0usize, 1, 254, 255, 256, 1000];
    for n in cases {
        let s = SjisString(vec![b'a'; n]);
        let mut w = Writer::default();
        w.cstring(&s);
        let mut r = Reader::new(&w.out);
        assert_eq!(r.cstring().unwrap(), s, "{n}");
        assert_eq!(r.pos, w.out.len(), "{n}");
    }
    let mut w = Writer::default();
    w.cstring(&SjisString(vec![b'a'; 255]));
    assert_eq!(&w.out[..3], &[0xff, 0xff, 0x00]);
}

#[test]
fn four_byte_string_length_is_rejected() {
    let mut r = Reader::new(&[0xff, 0xff, 0xff, 0, 0]);
    assert!(r.cstring().is_err());
}

// ----- 異常系 -----

#[test]
fn every_truncation_is_an_error_not_a_panic() {
    let bytes = everything().to_bytes();
    for n in 0..bytes.len() {
        assert!(Template::parse(&bytes[..n]).is_err(), "{n} バイトで成功");
    }
    assert!(Template::parse(&bytes).is_ok());
}

#[test]
fn rejects_structural_damage() {
    let good = sample().to_bytes();
    let with = |f: &dyn Fn(&mut Vec<u8>)| {
        let mut b = good.clone();
        f(&mut b);
        Template::parse(&b).unwrap_err().to_string()
    };
    // ヘッダ（TMP1 の直後は version u16, sub_version u16）
    assert!(with(&|b| b[4] = 2).contains("対応していない版"));
    assert!(with(&|b| b[6] = 5).contains("対応していない版"));
    assert!(with(&|b| b[0] = b'X').contains("TMP1"));
    // 並び: TMP1(0) 版(4) 副版(6) 入力区分(8) 向き(12) 横(16) 縦(18) CARD(20) 背景色(24) 個数(28) 最初の OBJ(32)
    assert!(with(&|b| b[20] = b'X').contains("CARD"));
    // オブジェクト数が大きすぎる
    assert!(with(&|b| b[28..32].copy_from_slice(&20_000u32.to_le_bytes())).contains("大きすぎ"));
    // OBJ タグ
    assert!(with(&|b| b[32] = b'X').contains("OBJ ヘッダ"));
    assert!(with(&|b| b[35] = b'9').contains("OBJ ヘッダ"));
    // 未知の種類
    assert!(with(&|b| b[36..40].copy_from_slice(&42u32.to_le_bytes())).contains("未知の種類 42"));
    // 末尾の余り
    assert!(with(&|b| b.push(0)).contains("未知のデータ"));
}

#[test]
fn errors_name_the_object_and_offset() {
    let mut b = sample().to_bytes();
    b.truncate(b.len() - 3);
    let e = Template::parse(&b).unwrap_err();
    assert!(e.message.contains("3 個目のオブジェクト"), "{e}");
    assert!(e.to_string().contains("バイト目"), "{e}");
}

#[test]
fn oversized_counts_are_rejected() {
    // 施工管理値の行数
    let mut t = Template::default();
    t.objects.push(cm_value(2));
    let mut bytes = t.to_bytes();
    let at = bytes.len() - 2 * 68 - 4;
    bytes[at..at + 4].copy_from_slice(&20_000u32.to_le_bytes());
    assert!(
        Template::parse(&bytes)
            .unwrap_err()
            .message
            .contains("行数")
    );
    // 多角形の点の数
    let mut t = Template::default();
    let mut poly = poly_bytes(&[]);
    let n = poly.len() - 4;
    poly[n..].copy_from_slice(&200_000u32.to_le_bytes());
    t.objects.push(passthrough(KIND_POLY, poly, 3));
    assert!(
        Template::parse(&t.to_bytes())
            .unwrap_err()
            .message
            .contains("点の数")
    );
}

// ----- オブジェクトの性質 -----

#[test]
fn object_versions_and_labels() {
    // (オブジェクト, 版, 表示名)
    let mut value = Object::new(
        Body::Caption(CaptionBody::new("", "工種", true, 12)),
        [0.0; 4],
    );
    let cases: Vec<(Object, u32, &str)> = vec![
        (
            Object::new(Body::Picture(PictureBody::default()), [0.0; 4]),
            10,
            "写真枠",
        ),
        (
            Object::new(
                Body::Caption(CaptionBody::new("a", "", false, 12)),
                [0.0; 4],
            ),
            10,
            "文字",
        ),
        (value.clone(), 10, "項目値"),
        (cm_value(1), 10, "施工管理値"),
        (cm_value(2), 11, "施工管理値"),
        (
            passthrough(KIND_DRAW_OBJECT, draw_object_bytes(), 1),
            10,
            "部品",
        ),
        (passthrough(KIND_DRAFT, draft_bytes(), 2), 11, "参考図"),
        (passthrough(KIND_RECT, rect_shape_bytes(0), 3), 12, "四角形"),
        (
            passthrough(KIND_RECT, rect_shape_bytes(1), 3),
            12,
            "角丸四角形",
        ),
        (passthrough(KIND_RECT, rect_shape_bytes(2), 3), 12, "楕円"),
        (passthrough(KIND_RECT, rect_shape_bytes(3), 3), 12, "直線"),
        (passthrough(KIND_RECT, rect_shape_bytes(4), 3), 12, "矢印"),
        (
            passthrough(KIND_POLY, poly_bytes(&[(0.0, 0.0)]), 3),
            12,
            "多角形",
        ),
        (passthrough(KIND_PATTERN, vec![0; 3], 3), 12, "図形"),
    ];
    for (o, version, label) in &cases {
        assert_eq!(o.object_version(), *version, "{label}");
        assert_eq!(o.kind_label(), *label);
    }
    value.tag_version = 3;
    assert_eq!(value.object_version(), 12);
}

#[test]
fn shape_view_reads_geometry_of_passthrough_shapes() {
    let rect = passthrough(KIND_RECT, rect_shape_bytes(1), 3);
    let v = rect.shape_view().unwrap();
    assert_eq!(v.shape, ShapeKind::RoundRect);
    assert_eq!(v.round, (0.3, 0.3));
    assert_eq!(v.base.brush_color, 0x00ffcc);
    assert!(v.points.is_empty());

    let pattern = passthrough(KIND_PATTERN, pattern_bytes(), 3);
    assert_eq!(pattern.shape_view().unwrap().shape, ShapeKind::Ellipse);

    let pts = [(0.0, 0.0), (1.0, 0.5), (0.5, 1.0)];
    let poly = passthrough(KIND_POLY, poly_bytes(&pts), 3);
    let v = poly.shape_view().unwrap();
    assert_eq!(v.shape, ShapeKind::Polygon);
    assert_eq!(v.points, pts);

    // 図形でないもの・読めないものは None
    assert!(
        passthrough(KIND_DRAW_OBJECT, draw_object_bytes(), 1)
            .shape_view()
            .is_none()
    );
    assert!(passthrough(KIND_RECT, vec![0; 3], 3).shape_view().is_none());
    assert!(
        passthrough(KIND_POLY, poly_bytes(&[]), 3)
            .shape_view()
            .unwrap()
            .points
            .is_empty()
    );
    assert!(sample().objects[0].shape_view().is_none());
}

#[test]
fn shape_kinds() {
    let cases = [
        (0, ShapeKind::Rectangle, "四角形"),
        (1, ShapeKind::RoundRect, "角丸四角形"),
        (2, ShapeKind::Ellipse, "楕円"),
        (3, ShapeKind::Line, "直線"),
        (4, ShapeKind::Arrow, "矢印"),
        (99, ShapeKind::Rectangle, "四角形"),
    ];
    for (v, kind, label) in cases {
        assert_eq!(ShapeKind::from_u32(v), kind);
        assert_eq!(kind.label(), label);
    }
    assert_eq!(ShapeKind::Polygon.label(), "多角形");
}

#[test]
fn passthrough_and_editable_flags() {
    let o = passthrough(KIND_RECT, rect_shape_bytes(0), 3);
    assert!(o.is_passthrough());
    assert!(!sample().objects[0].is_passthrough());
}

#[test]
#[should_panic(expected = "透過保持")]
fn passthrough_objects_cannot_be_created_from_scratch() {
    let _ = Object::new(Body::Passthrough(vec![]), [0.0; 4]);
}

#[test]
fn required_sub_version_never_goes_down() {
    let picture = |tag| {
        let mut o = Object::new(Body::Picture(PictureBody::default()), [0.0; 4]);
        o.tag_version = tag;
        o
    };
    let caption = Object::new(
        Body::Caption(CaptionBody::new("a", "", false, 12)),
        [0.0; 4],
    );
    let rect = passthrough(KIND_RECT, rect_shape_bytes(0), 3);
    // (ヘッダの値, オブジェクト, 期待)
    let cases = [
        (4u16, picture(2), 4u16),
        (1, picture(2), 3),
        (1, rect.clone(), 4),
        (3, rect, 4),
        (3, caption, 3),
    ];
    for (sub, o, expected) in cases {
        let mut t = Template::default();
        t.header.sub_version = sub;
        t.objects.push(o);
        assert_eq!(t.required_sub_version(), expected, "{sub}");
    }
    assert_eq!(Template::default().required_sub_version(), 2);
}

#[test]
fn to_bytes_writes_required_sub_version() {
    let mut t = Template::default();
    t.header.sub_version = 2;
    t.objects
        .push(passthrough(KIND_RECT, rect_shape_bytes(0), 3));
    let bytes = t.to_bytes();
    assert_eq!(u16::from_le_bytes([bytes[6], bytes[7]]), 4);
    assert_eq!(t.header.sub_version, 2, "元のヘッダは書き換えない");
}

// ----- 文字列・色・書式 -----

#[test]
fn sjis_strings() {
    let cases = [
        "",
        "abc",
        "写真タイトル",
        "ＭＳ Ｐゴシック",
        "撮影年月日 2026/11/09",
    ];
    for s in cases {
        let sj = SjisString::from_str(s);
        assert_eq!(sj.to_string_lossy(), s);
        assert_eq!(sj.is_empty(), s.is_empty());
        assert!(!sjis_unmappable(s));
    }
    assert!(sjis_unmappable("😀"));
    assert!(!sjis_unmappable("①"));
}

#[test]
fn fixed_strings_are_nul_terminated_and_truncated() {
    let f: [u8; 8] = string_to_fixed("ＭＳ ゴシック");
    assert_eq!(f[7], 0, "終端を必ず残す");
    assert_eq!(fixed_to_string(&f), "ＭＳ ゴ", "7 バイトに収まる分");
    let f: [u8; 32] = string_to_fixed("ＭＳ Ｐゴシック");
    assert_eq!(fixed_to_string(&f), "ＭＳ Ｐゴシック");
    assert_eq!(fixed_to_string(b"abc"), "abc");
    assert_eq!(fixed_to_string(b"ab\0cd"), "ab");
    assert_eq!(fixed_to_string(&[]), "");
}

#[test]
fn colorref_roundtrip() {
    let cases = [
        (0x0000_0000u32, [0u8, 0, 0]),
        (0x00ff_ffff, [255, 255, 255]),
        (0x0000_00ff, [255, 0, 0]),
        (0x00ff_0000, [0, 0, 255]),
        (0x0080_4020, [0x20, 0x40, 0x80]),
    ];
    for (c, rgb) in cases {
        assert_eq!(colorref_rgb(c), rgb, "{c:#x}");
        assert_eq!(rgb_colorref(rgb), c & 0x00ff_ffff, "{c:#x}");
    }
    assert_eq!(
        colorref_rgb(0xff12_3456),
        [0x56, 0x34, 0x12],
        "上位バイトは無視"
    );
}

#[test]
fn default_format_has_documented_values() {
    let f = default_format(14);
    assert_eq!(read_fmt_u16(&f, fmt_off::SIZE), 14);
    assert_eq!(read_fmt(&f, fmt_off::WEIGHT), 400);
    assert_eq!(f[fmt_off::HORIZONTAL], 1);
    assert_eq!(f[fmt_off::WRAP], 1);
    assert_eq!(f[fmt_off::ITALIC], 0);
    assert_eq!(read_fmt(&f, fmt_off::BG_COLOR), 0x00ff_ffff);
    assert_eq!(f.len(), FMT_LEN);
}

#[test]
fn fmt_accessors_roundtrip_and_stay_in_their_bytes() {
    let mut raw = default_format(12);
    let before = raw;
    {
        let mut f = Fmt(&mut raw);
        f.set_flag(fmt_off::ITALIC, true);
        f.set_u8(fmt_off::ALIGN, 2);
        f.set_u16(fmt_off::LINE_SPACE, 130);
        f.set_u32(fmt_off::TEXT_COLOR, 0x0012_3456);
        assert!(f.flag(fmt_off::ITALIC));
        assert_eq!(f.u8(fmt_off::ALIGN), 2);
        assert_eq!(f.u16(fmt_off::LINE_SPACE), 130);
        assert_eq!(f.u32(fmt_off::TEXT_COLOR), 0x0012_3456);
        f.set_flag(fmt_off::ITALIC, false);
    }
    let touched: Vec<usize> = (0..FMT_LEN).filter(|&k| raw[k] != before[k]).collect();
    let allowed: Vec<usize> = [0x06, 0x0e, 0x0f, 0x1c, 0x1d, 0x1e, 0x1f].into();
    for k in touched {
        assert!(allowed.contains(&k), "想定外のバイト {k:#x}");
    }
}

// ----- ヘッダ -----

#[test]
fn header_orientation_and_folder_names() {
    let cases = [
        (1u32, 1u16, 3u16, true, "TEMP1x3_V"),
        (0, 2, 2, false, "TEMP2x2_H"),
        (7, 4, 5, true, "TEMP4x5_V"),
    ];
    for (orientation, cols, rows, portrait, folder) in cases {
        let h = Header {
            orientation,
            cols,
            rows,
            ..Header::default()
        };
        assert_eq!(h.is_portrait(), portrait);
        assert_eq!(h.folder_name(), folder);
        let (c, r, o) = parse_folder_name(folder).unwrap();
        assert_eq!((c, r, o != 0), (cols, rows, portrait));
    }
}

#[test]
fn folder_name_parser_rejects_malformed_names() {
    for name in [
        "",
        "TEMP",
        "TEMP1x3",
        "TEMP1x3_X",
        "TEMPaxb_V",
        "TEMP1_3_V",
        "temp1x3_V",
    ] {
        assert_eq!(parse_folder_name(name), None, "{name:?}");
    }
    assert_eq!(parse_folder_name("TEMP10x20_v"), Some((10, 20, 1)));
    assert_eq!(parse_folder_name("TEMP1x1_h"), Some((1, 1, 0)));
}

#[test]
fn a4_constants() {
    assert!((A4_SHORT_INCH * MM_PER_INCH - 210.0).abs() < 1e-9);
    assert!((A4_LONG_INCH * MM_PER_INCH - 297.0).abs() < 1e-9);
    assert_eq!(EXPINF_TEXT, b"Ver7.10Template\r\n");
}

#[test]
fn parse_error_display_includes_offset() {
    let e = ParseError {
        offset: 12,
        message: "壊れています".into(),
    };
    assert_eq!(e.to_string(), "12 バイト目: 壊れています");
}

#[test]
fn border_width_table() {
    assert_eq!(BORDER_WIDTHS_MM, [0.1, 0.3, 0.7]);
}
