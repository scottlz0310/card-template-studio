//! 実機（本体）での差分実験用の、実物からの TPL 生成（Issue #18）。
//!
//! 実物の TMPEXP（`CARD_TPL_SAMPLES`）を土台に、調べたい値だけを変えた TPL を作る。
//! 手動実行専用（`#[ignore]`）。出力先は `CARD_TPL_EXPERIMENT_OUT`。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::*;

fn samples_dir() -> String {
    std::env::var("CARD_TPL_SAMPLES").expect("CARD_TPL_SAMPLES が未設定")
}

fn read_template(rel: &str) -> Template {
    let p = Path::new(&samples_dir()).join(rel);
    let data = std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    Template::parse(&data).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn write_template(out_dir: &Path, folder: &str, name: &str, t: &Template) -> PathBuf {
    let dir = out_dir.join(folder);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(format!("{name}.TPL"));
    std::fs::write(&p, t.to_bytes()).unwrap();
    p
}

#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP が要る。調査用の手動実行"]
fn dump_template() {
    let rel =
        std::env::var("CARD_TPL_DUMP").expect("CARD_TPL_DUMP（TMPEXP からの相対パス）が未設定");
    let t = read_template(&rel);
    println!("header: {:?} card_color={:#x}", t.header, t.card_color);
    for (i, o) in t.objects.iter().enumerate() {
        match &o.body {
            Body::Picture(p) => println!(
                "#{i} 写真枠 link={} rect={:?} b8={} bc={:#x} cc={:#x} d0={} d4={} d8={} e0={:#x} e8={} ec={:?} guid={:?} c4={:?} path={:?} dc={:?} e4={:?}",
                o.link,
                o.rect,
                p.frame_style,
                p.border_color,
                p.cc,
                p.d0,
                p.d4,
                p.d8,
                p.e0,
                p.e8,
                p.ec,
                p.guid.to_string_lossy(),
                p.c4.to_string_lossy(),
                p.path.to_string_lossy(),
                p.dc.to_string_lossy(),
                p.e4.to_string_lossy()
            ),
            Body::Caption(c) => {
                println!(
                    "#{i} 文字 link={} rect={:?} text={:?} field={:?} is_value={} t108={} border={} inset={} style={:?}",
                    o.link,
                    o.rect,
                    c.text.to_string_lossy(),
                    c.field.to_string_lossy(),
                    c.is_value,
                    c.t108,
                    c.border,
                    c.border_inset,
                    c.border_style
                );
                println!(
                    "    face={:?} value={:?} fmt={}",
                    fixed_to_string(&c.face),
                    c.value.to_string_lossy(),
                    c.fmt
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                );
            }
            _ => println!("#{i} {} link={} rect={:?}", o.kind_label(), o.link, o.rect),
        }
    }
}

#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP が要る。調査用の手動実行"]
fn picture_combinations() {
    let mut h: BTreeMap<String, usize> = BTreeMap::new();
    let root = samples_dir();
    for sub in std::fs::read_dir(&root).unwrap().flatten() {
        if !sub.path().is_dir() {
            continue;
        }
        for f in std::fs::read_dir(sub.path()).unwrap().flatten() {
            let p = f.path();
            if p.extension().map(|e| e.eq_ignore_ascii_case("tpl")) != Some(true) {
                continue;
            }
            let t = Template::parse(&std::fs::read(&p).unwrap()).unwrap();
            for o in &t.objects {
                if let Body::Picture(pb) = &o.body {
                    let key = format!(
                        "b8={} bc={:#x} cc={:#x} e8={} 大きさ=({}, {})",
                        pb.frame_style, pb.border_color, pb.cc, pb.e8, pb.d4, pb.d8
                    );
                    *h.entry(key).or_default() += 1;
                }
            }
        }
    }
    let mut rows: Vec<_> = h.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (k, n) in rows {
        println!("{n:>4}  {k}");
    }
}

/// 写真枠の `b8` / `bc` を枠ごとに変えた TPL（1x3 縦）を 1 つ作る。
/// 枠は左上から右へ 4 列 × 2 行に並べる（番号は並べた順）。
#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP と、CARD_TPL_EXPERIMENT_OUT が要る。調査用の手動実行"]
fn gen_picture_frame_experiment() {
    let out = PathBuf::from(
        std::env::var("CARD_TPL_EXPERIMENT_OUT").expect("CARD_TPL_EXPERIMENT_OUT が未設定"),
    );
    // COLORREF は 0x00BBGGRR
    let cases: [(u32, u32); 8] = [
        (0, 0x0),
        (0x11, 0x0000ff),
        (0x11, 0x00ff00),
        (0x11, 0xff0000),
        (0x00, 0x0000ff),
        (0x10, 0x0000ff),
        (0x01, 0x0000ff),
        (0x02, 0x0000ff),
    ];
    let mut t = read_template("TEMP1x3_V/写真のみ.TPL");
    let base = t.objects.remove(0);
    let Body::Picture(base_body) = &base.body else {
        panic!("写真のみ.TPL の先頭が写真枠でない")
    };
    t.objects.clear();
    for (i, (b8, bc)) in cases.iter().enumerate() {
        let (col, row) = ((i % 4) as f64, (i / 4) as f64);
        let (x, y) = (0.1 + col * 2.0, 0.15 + row * 1.8);
        let mut o = base.clone();
        o.rect = [x, y, x + 1.85, y + 1.65];
        let mut b = base_body.clone();
        b.frame_style = *b8;
        b.border_color = *bc;
        o.body = Body::Picture(b);
        t.objects.push(o);
    }
    let p = write_template(&out, "TEMP1x3_V", "実験_写真枠b8bc", &t);
    println!("{}", p.display());
}

/// 写真枠の枠線の太さの候補（`cc` / `d0` / `e0`）を枠ごとに変えた TPL を作る。
/// 全枠 `b8 = 0x11`（角丸＋枠線）, `bc` = 赤。枠は 4 列 × 2 行（左上から右へ）。
#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP と、CARD_TPL_EXPERIMENT_OUT が要る。調査用の手動実行"]
fn gen_picture_frame_border_experiment() {
    let out = PathBuf::from(
        std::env::var("CARD_TPL_EXPERIMENT_OUT").expect("CARD_TPL_EXPERIMENT_OUT が未設定"),
    );
    // (cc, d0, e0)
    let cases: [(u32, u32, u32); 8] = [
        (0, 0, 0xffff_ffff),
        (1, 0, 0xffff_ffff),
        (2, 0, 0xffff_ffff),
        (5, 0, 0xffff_ffff),
        (0, 1, 0xffff_ffff),
        (0, 5, 0xffff_ffff),
        (0, 0, 0),
        (0, 0, 5),
    ];
    let mut t = read_template("TEMP1x3_V/写真のみ.TPL");
    let base = t.objects.remove(0);
    let Body::Picture(base_body) = &base.body else {
        panic!("写真のみ.TPL の先頭が写真枠でない")
    };
    t.objects.clear();
    for (i, (cc, d0, e0)) in cases.iter().enumerate() {
        let (col, row) = ((i % 4) as f64, (i / 4) as f64);
        let (x, y) = (0.1 + col * 2.0, 0.15 + row * 1.8);
        let mut o = base.clone();
        o.rect = [x, y, x + 1.85, y + 1.65];
        let mut b = base_body.clone();
        b.frame_style = 0x11;
        b.border_color = 0x0000ff;
        (b.cc, b.d0, b.e0) = (*cc, *d0, *e0);
        o.body = Body::Picture(b);
        t.objects.push(o);
    }
    let p = write_template(&out, "TEMP1x3_V", "実験_写真枠太さ", &t);
    println!("{}", p.display());
}

/// `b8` の bit 4〜7（枠線まわりの値）を 0〜15 で全部試す TPL を作る（下位は 1 = 角丸）。
/// 全枠 `bc` = 赤。枠は 8 列 × 2 行（左上から右へ。1 行目が 0〜7、2 行目が 8〜15）。
#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP と、CARD_TPL_EXPERIMENT_OUT が要る。調査用の手動実行"]
fn gen_picture_frame_b8_nibble_experiment() {
    let out = PathBuf::from(
        std::env::var("CARD_TPL_EXPERIMENT_OUT").expect("CARD_TPL_EXPERIMENT_OUT が未設定"),
    );
    let mut t = read_template("TEMP1x3_V/写真のみ.TPL");
    let base = t.objects.remove(0);
    let Body::Picture(base_body) = &base.body else {
        panic!("写真のみ.TPL の先頭が写真枠でない")
    };
    t.objects.clear();
    for n in 0..16u32 {
        let (col, row) = ((n % 8) as f64, (n / 8) as f64);
        let (x, y) = (0.1 + col * 1.0, 0.15 + row * 1.8);
        let mut o = base.clone();
        o.rect = [x, y, x + 0.9, y + 1.65];
        let mut b = base_body.clone();
        b.frame_style = (n << 4) | 1;
        b.border_color = 0x0000ff;
        o.body = Body::Picture(b);
        t.objects.push(o);
    }
    let p = write_template(&out, "TEMP1x3_V", "実験_写真枠b8nibble", &t);
    println!("{}", p.display());
}

/// `b8` の下位 4 ビット（枠の形）を 0〜15 で全部試す TPL を作る（上位は 1 = 細い枠線）。
/// 全枠 `bc` = 赤。枠は 8 列 × 2 行（左上から右へ。1 行目が 0〜7、2 行目が 8〜15）。
#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP と、CARD_TPL_EXPERIMENT_OUT が要る。調査用の手動実行"]
fn gen_picture_frame_b8_shape_experiment() {
    let out = PathBuf::from(
        std::env::var("CARD_TPL_EXPERIMENT_OUT").expect("CARD_TPL_EXPERIMENT_OUT が未設定"),
    );
    let mut t = read_template("TEMP1x3_V/写真のみ.TPL");
    let base = t.objects.remove(0);
    let Body::Picture(base_body) = &base.body else {
        panic!("写真のみ.TPL の先頭が写真枠でない")
    };
    t.objects.clear();
    for n in 0..16u32 {
        let (col, row) = ((n % 8) as f64, (n / 8) as f64);
        let (x, y) = (0.1 + col * 1.0, 0.15 + row * 1.8);
        let mut o = base.clone();
        o.rect = [x, y, x + 0.9, y + 1.65];
        let mut b = base_body.clone();
        b.frame_style = 0x10 | n;
        b.border_color = 0x0000ff;
        o.body = Body::Picture(b);
        t.objects.push(o);
    }
    let p = write_template(&out, "TEMP1x3_V", "実験_写真枠b8shape", &t);
    println!("{}", p.display());
}

/// 書式の未確定バイト（+0x12 / +0x13 / +0x18）が実物のどの部品で変わるか。
/// 変わる部品の、文字・項目・色・背景・枠線などを並べて、関連を探す。
#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP が要る。調査用の手動実行"]
fn list_fmt_variants() {
    let root = samples_dir();
    let mut rows: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for sub in std::fs::read_dir(&root).unwrap().flatten() {
        if !sub.path().is_dir() {
            continue;
        }
        for f in std::fs::read_dir(sub.path()).unwrap().flatten() {
            let p = f.path();
            if p.extension().map(|e| e.eq_ignore_ascii_case("tpl")) != Some(true) {
                continue;
            }
            let t = Template::parse(&std::fs::read(&p).unwrap()).unwrap();
            let name = p.file_stem().unwrap().to_string_lossy().to_string();
            for (i, o) in t.objects.iter().enumerate() {
                let Body::Caption(c) = &o.body else { continue };
                let fm = &c.fmt;
                let key = format!(
                    "+12={:>2} +13={} +18={:>4}",
                    fm[0x12],
                    fm[0x13],
                    read_fmt(fm, 0x18) as i32
                );
                rows.entry(key).or_default().push(format!(
                    "{name}#{i} text={:?} field={:?} fill={} bg={:#08x} fg={:#08x} border={} bcol={:#08x} size={} weight={} wrap={} align={}",
                    c.text.to_string_lossy(),
                    c.field.to_string_lossy(),
                    fm[fmt_off::FILL],
                    read_fmt(fm, fmt_off::BG_COLOR),
                    read_fmt(fm, fmt_off::TEXT_COLOR),
                    c.border,
                    c.border_style.map(|s| s.0).unwrap_or(0),
                    read_fmt_u16(fm, fmt_off::SIZE),
                    read_fmt(fm, fmt_off::WEIGHT),
                    fm[fmt_off::WRAP],
                    fm[fmt_off::ALIGN],
                ));
            }
        }
    }
    for (k, v) in &rows {
        println!("\n## {k}（{} 件）", v.len());
        for line in v.iter().take(6) {
            println!("  {line}");
        }
    }
}

/// 書式（44 バイト）を書き換える 1 件分: (オフセット, 幅（バイト数）, 値)
type FmtEdit = (usize, usize, u32);

/// 書式を書き換えた文字項目（既定の書式 24pt・枠線あり・文字「あAbc」）を、
/// 4 列 × 行数分に並べた TPL（1x3 縦）を作る。番号は左上から右への並び順。
fn write_fmt_experiment(name: &str, cases: &[Vec<FmtEdit>]) {
    let out = PathBuf::from(
        std::env::var("CARD_TPL_EXPERIMENT_OUT").expect("CARD_TPL_EXPERIMENT_OUT が未設定"),
    );
    let mut t = read_template("TEMP1x3_V/写真のみ.TPL");
    t.objects.clear();
    for (i, edits) in cases.iter().enumerate() {
        let (col, row) = ((i % 4) as f64, (i / 4) as f64);
        let (x, y) = (0.1 + col * 2.0, 0.1 + row * 0.9);
        let mut c = CaptionBody::new("あAbc", "", false, 24);
        for (off, width, value) in edits {
            c.fmt[*off..*off + *width].copy_from_slice(&value.to_le_bytes()[..*width]);
        }
        t.objects
            .push(Object::new(Body::Caption(c), [x, y, x + 1.85, y + 0.8]));
    }
    let p = write_template(&out, "TEMP1x3_V", name, &t);
    println!("{}", p.display());
}

/// 1 回目: 未確定バイトを 1 か所ずつ（0x04 / 0x0a / 0x0c / 0x12 / 0x13 / 0x18 / 0x20 / 0x24）。
/// 結果: 0x04 = 1, 2 で効果が出る。0x09 は 0x08 の u16（文字サイズ）の上位バイトで、1 にすると 280pt。
#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP と、CARD_TPL_EXPERIMENT_OUT が要る。調査用の手動実行"]
fn gen_fmt_unknown_experiment() {
    let cases: Vec<Vec<FmtEdit>> = vec![
        vec![],
        vec![(0x04, 1, 1)],
        vec![(0x04, 1, 2)],
        vec![(0x09, 1, 1)],
        vec![(0x0a, 2, 0)],
        vec![(0x0a, 2, 15)],
        vec![(0x0c, 2, 0)],
        vec![(0x0c, 2, 10)],
        vec![(0x12, 1, 1)],
        vec![(0x12, 1, 63)],
        vec![(0x13, 1, 1)],
        vec![(0x18, 4, 0)],
        vec![(0x18, 4, 45)],
        vec![(0x18, 4, 135)],
        vec![(0x20, 4, 0xff_0000)],
        vec![(0x24, 4, 0x00_00ff)],
    ];
    write_fmt_experiment("実験_書式未確定", &cases);
}

/// 2 回目: 0x04 を 0〜7、および 0x04 = 1（影付き?）/ 2（袋文字?）で色・角度・間隔を変える。
#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP と、CARD_TPL_EXPERIMENT_OUT が要る。調査用の手動実行"]
fn gen_fmt_effect_experiment() {
    let mut cases: Vec<Vec<FmtEdit>> = (0..8).map(|v| vec![(0x04, 1, v)]).collect();
    cases.extend([
        vec![(0x04, 1, 1), (0x20, 4, 0xff_0000)],
        vec![(0x04, 1, 1), (0x24, 4, 0x00_00ff)],
        vec![(0x04, 1, 1), (0x18, 4, 0)],
        vec![(0x04, 1, 1), (0x18, 4, 135)],
        vec![(0x04, 1, 1), (0x0a, 2, 15)],
        vec![(0x04, 1, 1), (0x0c, 2, 10)],
        vec![(0x04, 1, 2), (0x20, 4, 0xff_0000)],
        vec![(0x04, 1, 2), (0x24, 4, 0x00_00ff)],
    ]);
    write_fmt_experiment("実験_書式効果", &cases);
}

/// 3 回目: 影（0x04 = 1）と袋文字（0x04 = 2）の大きさ・向きを決めるバイトを、大きな差で試す。
/// 結果の見方: 1 回目・2 回目で 0x0a = 15 にすると影が遠くなった（距離?）。0x18 は角度か。
#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP と、CARD_TPL_EXPERIMENT_OUT が要る。調査用の手動実行"]
fn gen_fmt_effect_size_experiment() {
    let shadow = (0x04, 1, 1);
    let outline = (0x04, 1, 2);
    let cases: Vec<Vec<FmtEdit>> = vec![
        vec![shadow, (0x0a, 2, 0)],
        vec![shadow, (0x0a, 2, 3)],
        vec![shadow, (0x0a, 2, 15)],
        vec![shadow, (0x0a, 2, 30)],
        vec![shadow, (0x0c, 2, 0)],
        vec![shadow, (0x0c, 2, 10)],
        vec![shadow, (0x0c, 2, 30)],
        vec![shadow, (0x0a, 2, 30), (0x0c, 2, 30)],
        vec![shadow, (0x0a, 2, 30), (0x18, 4, 0)],
        vec![shadow, (0x0a, 2, 30), (0x18, 4, 90)],
        vec![shadow, (0x0a, 2, 30), (0x18, 4, 135)],
        vec![shadow, (0x0a, 2, 30), (0x18, 4, 270)],
        vec![outline, (0x0a, 2, 30)],
        vec![outline, (0x0c, 2, 30)],
        vec![outline, (0x0c, 2, 0)],
        vec![shadow, (0x0a, 2, 30), (0x12, 1, 18), (0x13, 1, 1)],
    ];
    write_fmt_experiment("実験_書式効果2", &cases);
}

/// 書式の +0x12 / +0x13 が、フォント名や文字の向きなどと相関するか。
#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP が要る。調査用の手動実行"]
fn fmt_12_13_by_face() {
    let root = samples_dir();
    let mut h: BTreeMap<String, usize> = BTreeMap::new();
    for sub in std::fs::read_dir(&root).unwrap().flatten() {
        if !sub.path().is_dir() {
            continue;
        }
        for f in std::fs::read_dir(sub.path()).unwrap().flatten() {
            let p = f.path();
            if p.extension().map(|e| e.eq_ignore_ascii_case("tpl")) != Some(true) {
                continue;
            }
            let t = Template::parse(&std::fs::read(&p).unwrap()).unwrap();
            for o in &t.objects {
                let Body::Caption(c) = &o.body else { continue };
                let key = format!(
                    "face={:?} +12={:>2} +13={} 縦書き={} 斜体={} 太さ={}",
                    fixed_to_string(&c.face),
                    c.fmt[0x12],
                    c.fmt[0x13],
                    c.fmt[fmt_off::HORIZONTAL] == 0,
                    c.fmt[fmt_off::ITALIC],
                    read_fmt(&c.fmt, fmt_off::WEIGHT),
                );
                *h.entry(key).or_default() += 1;
            }
        }
    }
    let mut rows: Vec<_> = h.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (k, n) in rows {
        println!("{n:>4}  {k}");
    }
}

/// 書式の +0x12 / +0x13 / +0x18 が、同じテンプレートの中で揃っているか。
#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP が要る。調査用の手動実行"]
fn fmt_12_13_18_per_template() {
    let root = samples_dir();
    let (mut uniform, mut mixed) = (0, 0);
    for sub in std::fs::read_dir(&root).unwrap().flatten() {
        if !sub.path().is_dir() {
            continue;
        }
        for f in std::fs::read_dir(sub.path()).unwrap().flatten() {
            let p = f.path();
            if p.extension().map(|e| e.eq_ignore_ascii_case("tpl")) != Some(true) {
                continue;
            }
            let t = Template::parse(&std::fs::read(&p).unwrap()).unwrap();
            let mut kinds = std::collections::BTreeSet::new();
            for o in &t.objects {
                if let Body::Caption(c) = &o.body {
                    kinds.insert((c.fmt[0x12], c.fmt[0x13], read_fmt(&c.fmt, 0x18) as i32));
                }
            }
            match kinds.len() {
                0 | 1 => uniform += 1,
                _ => {
                    mixed += 1;
                    println!("混在: {} {kinds:?}", p.display());
                }
            }
        }
    }
    println!("テンプレート内で揃っている: {uniform} 件 / 混在: {mixed} 件");
}

/// 写真枠の `cc` / `d0` / `d4` / `d8` / `e8` を枠ごとに変えた TPL（1x3 縦、4 列 × 4 行）を作る。
/// 写真を貼ったときの見え方（切り抜き・収め方・向き）を比べるための実験。枠は全部同じ大きさ。
#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP と、CARD_TPL_EXPERIMENT_OUT が要る。調査用の手動実行"]
fn gen_picture_photo_experiment() {
    let out = PathBuf::from(
        std::env::var("CARD_TPL_EXPERIMENT_OUT").expect("CARD_TPL_EXPERIMENT_OUT が未設定"),
    );
    // (cc, d0, d4, d8, e8)
    const NONE: u32 = 0xffff_ffff;
    let cases: [(u32, u32, u32, u32, u32); 16] = [
        (0, 0, 0, 0, 3),
        (0, 0, 0, 0, 0),
        (0, 0, 0, 0, 1),
        (0, 0, 0, 0, 2),
        (0, 0, 0, 0, 4),
        (0, 0, 0, 0, 5),
        (0, 0, 0, 0, 6),
        (0, 0, 0, 0, 7),
        (1, 0, 0, 0, 3),
        (2, 0, 0, 0, 3),
        (3, 0, 0, 0, 3),
        (NONE, 0, 0, 0, 3),
        (0, 0, 1280, 960, 3),
        (0, 0, 100, 100, 3),
        (0, 0, 1280, 960, 4),
        (0, 1, 0, 0, 3),
    ];
    let mut t = read_template("TEMP1x3_V/写真のみ.TPL");
    let base = t.objects.remove(0);
    let Body::Picture(base_body) = &base.body else {
        panic!("写真のみ.TPL の先頭が写真枠でない")
    };
    t.objects.clear();
    for (i, (cc, d0, d4, d8, e8)) in cases.iter().enumerate() {
        let (col, row) = ((i % 4) as f64, (i / 4) as f64);
        let (x, y) = (0.1 + col * 2.0, 0.1 + row * 0.95);
        let mut o = base.clone();
        o.rect = [x, y, x + 1.9, y + 0.9];
        let mut b = base_body.clone();
        b.frame_style = 0x11;
        b.border_color = 0x0000ff;
        (b.cc, b.d0, b.d4, b.d8, b.e8) = (*cc, *d0, *d4, *d8, *e8);
        o.body = Body::Picture(b);
        t.objects.push(o);
    }
    let p = write_template(&out, "TEMP1x3_V", "実験_写真設定", &t);
    println!("{}", p.display());
}

/// 影の距離・袋文字の太さの単位を測る（1x1 のカードに並べる。1×3 への切り替えを避けるため）。
/// 1 行目: 影の距離 0 / 10 / 20 / 40 / 60（向き 0 = 真右）、2 行目: 距離 30 で向き 0 / 90 / 180 / 270 / -45、
/// 3 行目: 袋文字の太さ 3 / 10 / 20 / 40 / 60。枠（既定の枠線あり）の幅 1.25 in が、画面での寸法の物差しになる。
#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP と、CARD_TPL_EXPERIMENT_OUT が要る。調査用の手動実行"]
fn gen_fmt_unit_experiment() {
    let out = PathBuf::from(
        std::env::var("CARD_TPL_EXPERIMENT_OUT").expect("CARD_TPL_EXPERIMENT_OUT が未設定"),
    );
    let mut t = read_template("TEMP1x1_V/検証用テンプレート.TPL");
    t.objects.clear();
    let shadow = (0x04, 1, 1);
    let outline = (0x04, 1, 2);
    let red = (0x20, 4, 0x00_00ff);
    let red_edge = (0x24, 4, 0x00_00ff);
    let rows: Vec<Vec<Vec<FmtEdit>>> = vec![
        [0u32, 10, 20, 40, 60]
            .iter()
            .map(|&d| vec![shadow, red, (0x0a, 2, d)])
            .collect(),
        [0i32, 90, 180, 270, -45]
            .iter()
            .map(|&a| vec![shadow, red, (0x0a, 2, 30), (0x18, 4, a as u32)])
            .collect(),
        [3u32, 10, 20, 40, 60]
            .iter()
            .map(|&w| vec![outline, red_edge, (0x0c, 2, w)])
            .collect(),
    ];
    for (r, row) in rows.iter().enumerate() {
        for (c, edits) in row.iter().enumerate() {
            let (x, y) = (0.1 + c as f64 * 1.4, 0.1 + r as f64 * 0.9);
            let mut cap = CaptionBody::new("あ", "", false, 28);
            for (off, width, value) in edits {
                cap.fmt[*off..*off + *width].copy_from_slice(&value.to_le_bytes()[..*width]);
            }
            t.objects
                .push(Object::new(Body::Caption(cap), [x, y, x + 1.25, y + 0.7]));
        }
    }
    let p = write_template(&out, "TEMP1x1_V", "実験_効果単位", &t);
    println!("{}", p.display());
}

/// 影の距離・袋文字の太さが、文字の大きさに比例するかを調べる（`gen_fmt_unit_experiment` の続き）。
/// 1 行目: 距離 30・向き 0 の影、2 行目: 太さ 20 の袋文字。文字の大きさ 14 / 28 / 42 / 56pt。
#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP と、CARD_TPL_EXPERIMENT_OUT が要る。調査用の手動実行"]
fn gen_fmt_unit_by_size_experiment() {
    let out = PathBuf::from(
        std::env::var("CARD_TPL_EXPERIMENT_OUT").expect("CARD_TPL_EXPERIMENT_OUT が未設定"),
    );
    let mut t = read_template("TEMP1x1_V/検証用テンプレート.TPL");
    t.objects.clear();
    let red = (0x20, 4, 0x00_00ff);
    let red_edge = (0x24, 4, 0x00_00ff);
    let kinds: [Vec<FmtEdit>; 2] = [
        vec![(0x04, 1, 1), red, (0x0a, 2, 30), (0x18, 4, 0)],
        vec![(0x04, 1, 2), red_edge, (0x0c, 2, 20)],
    ];
    for (r, base) in kinds.iter().enumerate() {
        for (c, size) in [14u16, 28, 42, 56].iter().enumerate() {
            let (x, y) = (0.1 + c as f64 * 1.4, 0.1 + r as f64 * 1.5);
            let mut cap = CaptionBody::new("あ", "", false, *size);
            for (off, width, value) in base {
                cap.fmt[*off..*off + *width].copy_from_slice(&value.to_le_bytes()[..*width]);
            }
            t.objects
                .push(Object::new(Body::Caption(cap), [x, y, x + 1.25, y + 1.3]));
        }
    }
    let p = write_template(&out, "TEMP1x1_V", "実験_効果単位2", &t);
    println!("{}", p.display());
}

/// 施工管理値の未確定の項目（`c0` / `x144` / 「差の入力」の行）と、推定のままの値
/// （`name_style` = 1、`border_weight` = 1、項目文字揃え = 2）を、1x1 のカードに並べて見る（#18）。
/// 並び（左上から右へ、3 列）: 0 基準 / 1 c0=1 / 2 c0=2 / 3 x144=1 / 4 x144=2 / 5 name_style=1 /
/// 6 border_weight=1 / 7 差を表示（OBJ2・行なし）/ 8 差を表示+揃え 2 / 9 差の行 1 つ / 10 差の行 2 つ（先頭の値を変える）
#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP と、CARD_TPL_EXPERIMENT_OUT が要る。調査用の手動実行"]
fn gen_cm_unknown_experiment() {
    let out = PathBuf::from(
        std::env::var("CARD_TPL_EXPERIMENT_OUT").expect("CARD_TPL_EXPERIMENT_OUT が未設定"),
    );
    let mut t = read_template("TEMP1x1_V/検証用テンプレート.TPL");
    let base = t
        .objects
        .iter()
        .find_map(|o| match &o.body {
            Body::CmValue(c) => Some(c.clone()),
            _ => None,
        })
        .expect("施工管理値が無い");
    t.objects.clear();
    let row = |first: u32, a: &str, b: &str| {
        let mut r = [0u8; 68];
        r[0..4].copy_from_slice(&first.to_le_bytes());
        r[4..4 + a.len()].copy_from_slice(a.as_bytes());
        r[36..36 + b.len()].copy_from_slice(b.as_bytes());
        r
    };
    type Edit = Box<dyn Fn(&mut CmValueBody)>;
    let cases: Vec<(Edit, bool)> = vec![
        (Box::new(|_| {}), false),
        (Box::new(|c| c.c0 = 1), false),
        (Box::new(|c| c.c0 = 2), false),
        (Box::new(|c| c.x144 = 1), false),
        (Box::new(|c| c.x144 = 2), false),
        (Box::new(|c| c.name_style = 1), false),
        (
            Box::new(|c| {
                c.border_weight = 1;
                c.border_color = 0x00_00ff;
            }),
            false,
        ),
        (Box::new(|c| c.ext = Some((1, 0, vec![]))), true),
        (Box::new(|c| c.ext = Some((1, 2, vec![]))), true),
        (
            Box::new(move |c| c.ext = Some((1, 0, vec![row(1, "AB", "CD")]))),
            true,
        ),
        (
            Box::new(move |c| {
                c.ext = Some((1, 0, vec![row(0, "EF", "GH"), row(2, "IJ", "KL")]));
            }),
            true,
        ),
    ];
    for (i, (edit, obj2)) in cases.iter().enumerate() {
        let mut body = base.clone();
        body.count = 2;
        body.layout = 0;
        body.ext = None;
        edit(&mut body);
        let (x, y) = (0.1 + (i % 3) as f64 * 2.6, 0.1 + (i / 3) as f64 * 1.3);
        let mut o = Object::new(Body::CmValue(body), [x, y, x + 2.5, y + 1.2]);
        if *obj2 {
            o.tag_version = 2;
        }
        t.objects.push(o);
    }
    let p = write_template(&out, "TEMP1x1_V", "実験_施工管理値", &t);
    println!("{}", p.display());
}

/// 施工管理値の項目文字揃え（ext の 2 番目）と `c0` の、より大きな値を見る（`gen_cm_unknown_experiment` の続き）。
/// 並び: 0〜3 揃え 0 / 1 / 2 / 3、4〜7 c0 = 3 / 4 / 0x100 / 0xffffffff、8〜9 x144 = 0x100 / 0xffffffff
#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP と、CARD_TPL_EXPERIMENT_OUT が要る。調査用の手動実行"]
fn gen_cm_unknown_experiment2() {
    let out = PathBuf::from(
        std::env::var("CARD_TPL_EXPERIMENT_OUT").expect("CARD_TPL_EXPERIMENT_OUT が未設定"),
    );
    let mut t = read_template("TEMP1x1_V/検証用テンプレート.TPL");
    let base = t
        .objects
        .iter()
        .find_map(|o| match &o.body {
            Body::CmValue(c) => Some(c.clone()),
            _ => None,
        })
        .expect("施工管理値が無い");
    t.objects.clear();
    type Edit = Box<dyn Fn(&mut CmValueBody)>;
    let mut cases: Vec<Edit> = Vec::new();
    for a in [0u32, 1, 2, 3] {
        cases.push(Box::new(move |c| c.ext = Some((1, a, vec![]))));
    }
    for v in [3u32, 4, 0x100, 0xffff_ffff] {
        cases.push(Box::new(move |c| c.c0 = v));
    }
    for v in [0x100u32, 0xffff_ffff] {
        cases.push(Box::new(move |c| c.x144 = v));
    }
    for (i, edit) in cases.iter().enumerate() {
        let mut body = base.clone();
        body.count = 1;
        body.layout = 0;
        body.ext = None;
        edit(&mut body);
        let obj2 = body.ext.is_some();
        let (x, y) = (0.1 + (i % 3) as f64 * 2.6, 0.1 + (i / 3) as f64 * 1.0);
        let mut o = Object::new(Body::CmValue(body), [x, y, x + 2.5, y + 0.9]);
        if obj2 {
            o.tag_version = 2;
        }
        t.objects.push(o);
    }
    let p = write_template(&out, "TEMP1x1_V", "実験_施工管理値2", &t);
    println!("{}", p.display());
}

/// `gen_cm_unknown_experiment2` の 1 枚が、本体で読み込めなかった（プレビューが空で OK が押せない）ため、
/// 値の組ごとに別のファイルへ分けて、どの値が弾かれるかを調べる。
#[test]
#[ignore = "CARD_TPL_SAMPLES に実物の TMPEXP と、CARD_TPL_EXPERIMENT_OUT が要る。調査用の手動実行"]
fn gen_cm_reject_experiment() {
    let out = PathBuf::from(
        std::env::var("CARD_TPL_EXPERIMENT_OUT").expect("CARD_TPL_EXPERIMENT_OUT が未設定"),
    );
    let base_t = read_template("TEMP1x1_V/検証用テンプレート.TPL");
    let base = base_t
        .objects
        .iter()
        .find_map(|o| match &o.body {
            Body::CmValue(c) => Some(c.clone()),
            _ => None,
        })
        .expect("施工管理値が無い");
    type Edit = Box<dyn Fn(&mut CmValueBody)>;
    let groups: Vec<(&str, Vec<Edit>)> = vec![
        (
            "rj1_align23",
            vec![
                Box::new(|c| c.ext = Some((1, 2, vec![]))),
                Box::new(|c| c.ext = Some((1, 3, vec![]))),
            ],
        ),
        (
            "rj2_c0small",
            vec![
                Box::new(|c| c.c0 = 3),
                Box::new(|c| c.c0 = 4),
                Box::new(|c| c.c0 = 0x100),
            ],
        ),
        ("rj3_c0max", vec![Box::new(|c| c.c0 = 0xffff_ffff)]),
        ("rj4_x144_100", vec![Box::new(|c| c.x144 = 0x100)]),
        ("rj5_x144max", vec![Box::new(|c| c.x144 = 0xffff_ffff)]),
        ("rj6_base", vec![Box::new(|_| {})]),
    ];
    for (name, edits) in &groups {
        let mut t = base_t.clone();
        t.objects.clear();
        for (i, edit) in edits.iter().enumerate() {
            let mut body = base.clone();
            body.count = 1;
            body.layout = 0;
            body.ext = None;
            edit(&mut body);
            let obj2 = body.ext.is_some();
            let y = 0.1 + i as f64 * 1.0;
            let mut o = Object::new(Body::CmValue(body), [0.1, y, 2.6, y + 0.9]);
            if obj2 {
                o.tag_version = 2;
            }
            t.objects.push(o);
        }
        println!("{}", write_template(&out, "TEMP1x1_V", name, &t).display());
    }
}
