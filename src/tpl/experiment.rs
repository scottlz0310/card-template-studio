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
                "#{i} 写真枠 link={} rect={:?} b8={} bc={:#x} cc={:#x} d4={} d8={} e0={:#x} e8={} ec={:?}",
                o.link, o.rect, p.frame_style, p.border_color, p.cc, p.d4, p.d8, p.e0, p.e8, p.ec
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
