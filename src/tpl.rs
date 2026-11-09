//! 現場編集長アルバム（GhAlbum.exe）のカードテンプレート（.TPL）形式の読み書き。
//!
//! 形式の根拠は MECHANICS.md（関数アドレス付き）を参照。未解明の値も含めて
//! 読み込んだバイトをそのまま保持し、編集しない限り同じバイト列で書き戻す。

use encoding_rs::SHIFT_JIS;
use std::fmt;

/// 1 インチ = 25.4 mm（座標はインチで保存されている）
pub const MM_PER_INCH: f64 = 25.4;

pub const KIND_PICTURE: u32 = 2;
pub const KIND_CAPTION: u32 = 3;
pub const KIND_DRAW_OBJECT: u32 = 4;
pub const KIND_CM_VALUE: u32 = 5;
pub const KIND_DRAFT: u32 = 6;
pub const KIND_RECT: u32 = 7;
pub const KIND_PATTERN: u32 = 8;
pub const KIND_POLY: u32 = 9;

/// 本体のエクスポートが書く expinf の中身（0x598d94 付近の文字列 "Ver7" + ".10Template"）
pub const EXPINF_TEXT: &[u8] = b"Ver7.10Template\r\n";

#[derive(Debug)]
pub struct ParseError {
    pub offset: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} バイト目: {}", self.offset, self.message)
    }
}

impl std::error::Error for ParseError {}

type Result<T> = std::result::Result<T, ParseError>;

// ---------------------------------------------------------------------------
// 文字列（cp932）

/// cp932 のバイト列として保持する文字列。MFC CArchive と同じく長さ前置で保存される。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SjisString(pub Vec<u8>);

impl SjisString {
    pub fn from_str(s: &str) -> Self {
        let (bytes, _, _) = SHIFT_JIS.encode(s);
        Self(bytes.into_owned())
    }
    pub fn to_string_lossy(&self) -> String {
        SHIFT_JIS
            .decode_without_bom_handling(&self.0)
            .0
            .into_owned()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// 固定長の cp932 文字列（フォント名 32 バイトなど）。終端以降のバイトも保持する。
pub fn fixed_to_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    SHIFT_JIS
        .decode_without_bom_handling(&bytes[..end])
        .0
        .into_owned()
}

pub fn string_to_fixed<const N: usize>(s: &str) -> [u8; N] {
    let mut out = [0u8; N];
    let (bytes, _, _) = SHIFT_JIS.encode(s);
    let n = bytes.len().min(N - 1);
    out[..n].copy_from_slice(&bytes[..n]);
    out
}

/// cp932 で表せない文字を含むかどうか
pub fn sjis_unmappable(s: &str) -> bool {
    SHIFT_JIS.encode(s).2
}

// ---------------------------------------------------------------------------
// 読み書きの補助

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    fn err(&self, message: impl Into<String>) -> ParseError {
        ParseError {
            offset: self.pos,
            message: message.into(),
        }
    }
    fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.pos + n > self.data.len() {
            return Err(self.err(format!(
                "{n} バイト読もうとしてファイルの終わりに達しました"
            )));
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut a = [0u8; N];
        a.copy_from_slice(self.bytes(N)?);
        Ok(a)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.array()?))
    }
    fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_le_bytes(self.array()?))
    }
    /// CString: 長さ 1 バイト（0xFF のときは続く u16 が長さ）＋ 本体（0x45c000 / 0x45be30）
    fn cstring(&mut self) -> Result<SjisString> {
        let mut n = self.u8()? as usize;
        if n == 0xff {
            n = self.u16()? as usize;
            if n == 0xffff {
                return Err(self.err("4 バイト長の文字列には対応していません"));
            }
        }
        Ok(SjisString(self.bytes(n)?.to_vec()))
    }
    fn tag(&mut self) -> Result<[u8; 4]> {
        self.array()
    }
}

#[derive(Default)]
struct Writer {
    out: Vec<u8>,
}

impl Writer {
    fn bytes(&mut self, b: &[u8]) {
        self.out.extend_from_slice(b);
    }
    fn u8(&mut self, v: u8) {
        self.out.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.bytes(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.bytes(&v.to_le_bytes());
    }
    fn f64(&mut self, v: f64) {
        self.bytes(&v.to_le_bytes());
    }
    fn cstring(&mut self, s: &SjisString) {
        let n = s.0.len();
        if n < 0xff {
            self.u8(n as u8);
        } else {
            self.u8(0xff);
            self.u16(n.min(0xfffe) as u16);
        }
        self.bytes(&s.0[..n.min(0xfffe)]);
    }
}

// ---------------------------------------------------------------------------
// ヘッダ

/// 'TMP1' ヘッダ（20 バイト, 0x445ca0 が組み立て, 0x444f00 が検査）
#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    pub version: u16,
    /// 1〜4。カード内オブジェクトの最大版 10→2, 11→3, 12→4（0x445000）
    pub sub_version: u16,
    /// GetEnterKbn の戻り値（入力区分・要領の種類と推定）
    pub enter_kind: u32,
    /// 0 = 横（H）, 1 = 縦（V）
    pub orientation: u32,
    /// 横の分割数
    pub cols: u16,
    /// 縦の分割数
    pub rows: u16,
}

impl Default for Header {
    fn default() -> Self {
        Self {
            version: 1,
            sub_version: 2,
            enter_kind: 15,
            orientation: 1,
            cols: 1,
            rows: 3,
        }
    }
}

impl Header {
    pub fn is_portrait(&self) -> bool {
        self.orientation != 0
    }
    /// 本体のテンプレートフォルダ名（"TEMP%dx%d_%s"）
    pub fn folder_name(&self) -> String {
        format!(
            "TEMP{}x{}_{}",
            self.cols,
            self.rows,
            if self.is_portrait() { "V" } else { "H" }
        )
    }
}

pub const A4_SHORT_INCH: f64 = 210.0 / MM_PER_INCH;
pub const A4_LONG_INCH: f64 = 297.0 / MM_PER_INCH;

/// フォルダ名 "TEMP1x3_V" から分割数と向きを読む
pub fn parse_folder_name(name: &str) -> Option<(u16, u16, u32)> {
    let rest = name.strip_prefix("TEMP")?;
    let (dims, o) = rest.split_once('_')?;
    let (c, r) = dims.split_once('x')?;
    let orientation = match o {
        "V" | "v" => 1,
        "H" | "h" => 0,
        _ => return None,
    };
    Some((c.parse().ok()?, r.parse().ok()?, orientation))
}

// ---------------------------------------------------------------------------
// 文字の書式（0x407310 が既定値を設定する 0x6c バイト構造のうち、ファイルに入る 44 バイト）

pub const FMT_LEN: usize = 44;

/// 書式 44 バイトの各位置（構造体 +0x40 からの相対）
pub mod fmt_off {
    pub const ITALIC: usize = 0x00; // lfItalic（0x4e9450）
    pub const STRIKE: usize = 0x01; // lfStrikeOut
    pub const UNDERLINE: usize = 0x02; // lfUnderline
    pub const HORIZONTAL: usize = 0x03; // 0 = 縦書き（0x4ea200）
    pub const WRAP: usize = 0x05; // 自動折り返し（0x4d80e0）
    pub const ALIGN: usize = 0x06; // 0 左, 1 中央, 2 右（0x407790）
    pub const FILL: usize = 0x07; // 背景色あり（0x408b30, 0x4ea200）
    pub const SIZE: usize = 0x08; // u16 文字サイズ pt
    pub const LINE_SPACE: usize = 0x0e; // u16 行間（文字サイズに対する %）
    pub const CHAR_SPACE: usize = 0x10; // u16 文字間（文字サイズに対する %）
    pub const WEIGHT: usize = 0x14; // i32 太さ（400 標準 / 700 太字）
    pub const TEXT_COLOR: usize = 0x1c; // COLORREF
    pub const BG_COLOR: usize = 0x28; // COLORREF
}

/// 0x407310 の既定値（文字サイズは引数）
pub fn default_format(size_pt: u16) -> [u8; FMT_LEN] {
    let mut f = [0u8; FMT_LEN];
    f[0..4].copy_from_slice(&0x0100_0000u32.to_le_bytes());
    f[4..8].copy_from_slice(&0x0000_0100u32.to_le_bytes());
    f[8..10].copy_from_slice(&size_pt.to_le_bytes());
    f[0x0a..0x0e].copy_from_slice(&0x0003_0007u32.to_le_bytes());
    f[0x14..0x18].copy_from_slice(&400i32.to_le_bytes());
    f[0x18..0x1c].copy_from_slice(&(-45i32).to_le_bytes());
    f[0x20..0x24].copy_from_slice(&0x0080_8080u32.to_le_bytes());
    f[0x24..0x28].copy_from_slice(&0x0080_8080u32.to_le_bytes());
    f[0x28..0x2c].copy_from_slice(&0x00ff_ffffu32.to_le_bytes());
    f
}

/// 書式 44 バイトを読み書きするための小さな包み
pub struct Fmt<'a>(pub &'a mut [u8; FMT_LEN]);

impl Fmt<'_> {
    pub fn flag(&self, off: usize) -> bool {
        self.0[off] != 0
    }
    pub fn set_flag(&mut self, off: usize, v: bool) {
        self.0[off] = v as u8;
    }
    pub fn u8(&self, off: usize) -> u8 {
        self.0[off]
    }
    pub fn set_u8(&mut self, off: usize, v: u8) {
        self.0[off] = v;
    }
    pub fn u16(&self, off: usize) -> u16 {
        u16::from_le_bytes([self.0[off], self.0[off + 1]])
    }
    pub fn set_u16(&mut self, off: usize, v: u16) {
        self.0[off..off + 2].copy_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&self, off: usize) -> u32 {
        u32::from_le_bytes(self.0[off..off + 4].try_into().unwrap())
    }
    pub fn set_u32(&mut self, off: usize, v: u32) {
        self.0[off..off + 4].copy_from_slice(&v.to_le_bytes());
    }
}

pub fn read_fmt(f: &[u8; FMT_LEN], off: usize) -> u32 {
    u32::from_le_bytes(f[off..off + 4].try_into().unwrap())
}

pub fn read_fmt_u16(f: &[u8; FMT_LEN], off: usize) -> u16 {
    u16::from_le_bytes([f[off], f[off + 1]])
}

// ---------------------------------------------------------------------------
// オブジェクト本体

/// 写真枠（種類 2, CPictureObject 0x42f840 / 0x42d610）
#[derive(Clone, Debug, PartialEq)]
pub struct PictureBody {
    pub crop: [u8; 32],
    pub b8: u32,
    pub bc: u32,
    /// 写真の GUID（テンプレートでは空か "0"）
    pub guid: SjisString,
    pub c4: SjisString,
    /// 写真ファイルのパス
    pub path: SjisString,
    pub cc: u32,
    pub d0: u32,
    pub d4: u32,
    pub d8: u32,
    pub dc: SjisString,
    pub e0: u32,
    pub e4: SjisString,
    pub e8: u32,
    /// 版 11 以上（OBJ2/OBJ3）のときだけ保存される
    pub ec: Option<u32>,
}

impl Default for PictureBody {
    fn default() -> Self {
        Self {
            crop: [0; 32],
            b8: 0x11,
            bc: 0xff,
            guid: SjisString::default(),
            c4: SjisString::default(),
            path: SjisString::default(),
            cc: 0,
            d0: 0,
            d4: 0,
            d8: 0,
            dc: SjisString::default(),
            e0: 0xffff_ffff,
            e4: SjisString::default(),
            e8: 3,
            ec: None,
        }
    }
}

/// 文字項目（種類 3, CCaptionObject 0x407400 / 0x408610）
#[derive(Clone, Debug, PartialEq)]
pub struct CaptionBody {
    /// 表示する文字（項目名ラベルの文字）
    pub text: SjisString,
    pub face: [u8; 32],
    pub fmt: [u8; FMT_LEN],
    /// アルバム上の値（テンプレートでは "0"）
    pub value: SjisString,
    /// データベースの項目名（"写真タイトル" など）
    pub field: SjisString,
    /// +0x108（意味未確定, 0/1）
    pub t108: i32,
    /// +0x10c: 1 = 項目値（データを表示）, 0 = 項目名（固定の文字）
    pub is_value: i32,
    /// +0x110: 枠線あり（0x408b30, 読み込み時 0..1 に丸める）
    pub border: i32,
    /// +0x114: 0..2（1/2 で枠を 0.05/0.1 内側へ, 0x408b30）
    pub border_inset: i32,
    /// +0x118 / +0x11c: 版 2 以上でのみ保存（枠線色, 太さ 0/1/2 = 0.1/0.3/0.7mm）
    pub border_style: Option<(u32, i32)>,
}

impl CaptionBody {
    pub fn new(text: &str, field: &str, is_value: bool, size_pt: u16) -> Self {
        Self {
            text: SjisString::from_str(text),
            face: string_to_fixed("ＭＳ Ｐゴシック"),
            fmt: default_format(size_pt),
            value: SjisString::from_str("0"),
            field: SjisString::from_str(field),
            t108: 0,
            is_value: is_value as i32,
            border: 1,
            border_inset: 0,
            border_style: Some((0, 0)),
        }
    }
}

/// 施工管理値（種類 5, CCMValueObject 0x40f8a0）
#[derive(Clone, Debug, PartialEq)]
pub struct CmValueBody {
    pub text: SjisString,
    pub c0: u32,
    pub c4: u32,
    pub x138: u32,
    pub x13c: u32,
    pub x148: u32,
    pub face: [u8; 32],
    pub fmt: [u8; FMT_LEN],
    pub x140: u32,
    pub x154: u32,
    pub x150: u32,
    pub x14c: u32,
    pub x144: u32,
    /// 版 11 以上: +0x15c, +0x160, 行（各 68 バイト: u32 + 32 + 32）
    pub ext: Option<(u32, u32, Vec<[u8; 68]>)>,
}

/// 図形の共通部分（CDrawObj 0x421230 / 0x421170。MFC DRAWCLI と同じ並び）。表示のためだけに読む
#[derive(Clone, Debug, PartialEq)]
pub struct DrawBase {
    pub raw_90: u32,
    pub raw_94: u32,
    pub raw_98: u8,
    /// +0x9c: 線あり
    pub pen: u32,
    /// LOGPEN: 線種, 太さ x (pt, 0x421380 で /72), 太さ y, 色
    pub pen_style: u32,
    pub pen_width: i32,
    pub pen_width_y: i32,
    pub pen_color: u32,
    /// +0xb0: 塗りつぶしあり
    pub brush: u32,
    /// LOGBRUSH: 種類, 色, ハッチ
    pub brush_style: u32,
    pub brush_color: u32,
    pub brush_hatch: u32,
}

/// 図形の形（CDrawRect +0xc0, 0x4247a0 の分岐）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Rectangle,
    RoundRect,
    Ellipse,
    Line,
    Arrow,
    Polygon,
}

impl ShapeKind {
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => Self::RoundRect,
            2 => Self::Ellipse,
            3 => Self::Line,
            4 => Self::Arrow,
            _ => Self::Rectangle,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Rectangle => "四角形",
            Self::RoundRect => "角丸四角形",
            Self::Ellipse => "楕円",
            Self::Line => "直線",
            Self::Arrow => "矢印",
            Self::Polygon => "多角形",
        }
    }
}

/// 透過保持している図形を画面に描くための読み取り結果
#[derive(Clone, Debug)]
pub struct ShapeView {
    pub shape: ShapeKind,
    pub base: DrawBase,
    /// 角丸の比率（幅・高さに掛ける）
    pub round: (f64, f64),
    /// 多角形の点（インチ）
    pub points: Vec<(f64, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    Picture(PictureBody),
    Caption(CaptionBody),
    CmValue(CmValueBody),
    /// 種類 4・6・7・8・9（部品・参考図・図形）。サンプルに無く編集対象外なので、
    /// 読み込んだ本体のバイト列をそのまま保持して書き戻す。
    Passthrough(Vec<u8>),
}

/// カード上の 1 つのオブジェクト（44 バイトのヘッダ + 本体）
#[derive(Clone, Debug, PartialEq)]
pub struct Object {
    /// タグ 'OBJn' の n（1..3）。読み込み時は +0x5c に入る（0x41af10）
    pub tag_version: u8,
    pub kind: u32,
    /// ヘッダ 2 番目の値（+0x88）。文字項目がどの写真に属するか（1 始まり, 0 = なし）
    pub link: u32,
    /// left, top, right, bottom（インチ。線は向きを保つため正規化しない）
    pub rect: [f64; 4],
    pub body: Body,
    /// OBJ3 のときだけ本体の後ろに続く u32（+0x8c, 0x40b200 / 0x40d230）
    pub trailer: Option<u32>,
}

impl Object {
    /// 新しい写真枠・文字項目を作る（タグの版はサンプルと本体の書き方に合わせる:
    /// 写真枠は版 10 = 'OBJ1'（0x42ae80）、文字項目は 'OBJ2'（0x407660））
    pub fn new(body: Body, rect: [f64; 4]) -> Self {
        let (kind, tag_version) = match &body {
            Body::Picture(_) => (KIND_PICTURE, 1),
            Body::Caption(_) => (KIND_CAPTION, 2),
            Body::CmValue(_) => (KIND_CM_VALUE, 1),
            Body::Passthrough(_) => unreachable!("透過保持のオブジェクトは読み込みでしか作らない"),
        };
        Self {
            tag_version,
            kind,
            link: 0,
            rect,
            body,
            trailer: None,
        }
    }

    /// 本体が付ける「版」（10/11/12）。ヘッダ sub_version の計算に使う
    pub fn object_version(&self) -> u32 {
        match (&self.body, self.kind) {
            // 図形の版は常に 12（0x421050）
            (Body::Passthrough(_), KIND_RECT | KIND_PATTERN | KIND_POLY) => 12,
            // 文字項目は +0x8c==1 のとき 12、それ以外 10（0x41af30）。'OBJ2' は 10 の印
            (Body::Caption(_), _) => {
                if self.tag_version >= 3 {
                    12
                } else {
                    10
                }
            }
            _ => match self.tag_version {
                0 | 1 => 10,
                2 => 11,
                _ => 12,
            },
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match (&self.body, self.kind) {
            (Body::Picture(_), _) => "写真枠",
            (Body::Caption(c), _) => {
                if c.is_value != 0 {
                    "項目値"
                } else {
                    "文字"
                }
            }
            (Body::CmValue(_), _) => "施工管理値",
            (Body::Passthrough(_), KIND_DRAW_OBJECT) => "部品",
            (Body::Passthrough(_), KIND_DRAFT) => "参考図",
            (Body::Passthrough(_), _) => {
                self.shape_view().map(|s| s.shape.label()).unwrap_or("図形")
            }
        }
    }

    /// 透過保持していて編集できないオブジェクトか
    pub fn is_passthrough(&self) -> bool {
        matches!(self.body, Body::Passthrough(_))
    }

    pub fn normalized(&self) -> [f64; 4] {
        let [l, t, r, b] = self.rect;
        [l.min(r), t.min(b), l.max(r), t.max(b)]
    }

    /// 透過保持の図形（種類 7・8・9）を画面に描くために読む。読めなければ None
    pub fn shape_view(&self) -> Option<ShapeView> {
        let Body::Passthrough(raw) = &self.body else {
            return None;
        };
        let mut r = Reader::new(raw);
        let base = read_base(&mut r).ok()?;
        match self.kind {
            KIND_RECT | KIND_PATTERN => {
                let shape = ShapeKind::from_u32(r.u32().ok()?);
                for _ in 0..4 {
                    r.u32().ok()?;
                }
                let round = (r.f64().ok()?, r.f64().ok()?);
                Some(ShapeView {
                    shape,
                    base,
                    round,
                    points: Vec::new(),
                })
            }
            KIND_POLY => {
                let n = r.u32().ok()? as usize;
                let mut points = Vec::with_capacity(n.min(10_000));
                for _ in 0..n.min(10_000) {
                    points.push((r.f64().ok()?, r.f64().ok()?));
                }
                Some(ShapeView {
                    shape: ShapeKind::Polygon,
                    base,
                    round: (0.0, 0.0),
                    points,
                })
            }
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// テンプレート全体

#[derive(Clone, Debug, PartialEq)]
pub struct Template {
    pub header: Header,
    /// カードの背景色（CARD 2 番目の値, +0x28）
    pub card_color: u32,
    pub objects: Vec<Object>,
}

impl Default for Template {
    fn default() -> Self {
        Self {
            header: Header::default(),
            card_color: 0x00ff_ffff,
            objects: Vec::new(),
        }
    }
}

impl Template {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        if &r.tag()? != b"TMP1" {
            return Err(ParseError {
                offset: 0,
                message: "テンプレートファイルではありません（先頭が TMP1 ではない）".into(),
            });
        }
        let version = r.u16()?;
        let sub_version = r.u16()?;
        if version != 1 || sub_version > 4 {
            return Err(r.err(format!("対応していない版です（{version}.{sub_version}）")));
        }
        let enter_kind = r.u32()?;
        let orientation = r.u32()?;
        let cols = r.u16()?;
        let rows = r.u16()?;
        let header = Header {
            version,
            sub_version,
            enter_kind,
            orientation,
            cols,
            rows,
        };

        if &r.tag()? != b"CARD" {
            return Err(r.err("CARD が見つかりません"));
        }
        let card_color = r.u32()?;
        let count = r.u32()? as usize;
        if count > 10_000 {
            return Err(r.err(format!("オブジェクト数が大きすぎます（{count}）")));
        }
        let mut objects = Vec::with_capacity(count);
        for i in 0..count {
            objects.push(read_object(&mut r).map_err(|e| ParseError {
                offset: e.offset,
                message: format!("{} 個目のオブジェクト: {}", i + 1, e.message),
            })?);
        }
        if r.pos != data.len() {
            return Err(r.err(format!(
                "末尾に {} バイトの未知のデータがあります",
                data.len() - r.pos
            )));
        }
        Ok(Self {
            header,
            card_color,
            objects,
        })
    }

    /// 本体と同じ並びで書き出す
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer::default();
        let h = &self.header;
        w.bytes(b"TMP1");
        w.u16(h.version);
        w.u16(self.required_sub_version());
        w.u32(h.enter_kind);
        w.u32(h.orientation);
        w.u16(h.cols);
        w.u16(h.rows);
        w.bytes(b"CARD");
        w.u32(self.card_color);
        w.u32(self.objects.len() as u32);
        for o in &self.objects {
            write_object(&mut w, o);
        }
        w.out
    }

    /// sub_version は読み込んだ値を保ち、版 11/12 のオブジェクトがあるときだけ 3/4 へ上げる（0x445000）
    pub fn required_sub_version(&self) -> u16 {
        let max = self
            .objects
            .iter()
            .map(Object::object_version)
            .max()
            .unwrap_or(10);
        let needed = match max {
            11 => 3,
            v if v >= 12 => 4,
            _ => 2,
        };
        if needed > 2 {
            self.header.sub_version.max(needed)
        } else {
            self.header.sub_version
        }
    }
}

fn read_object(r: &mut Reader) -> Result<Object> {
    let start = r.pos;
    let tag = r.tag()?;
    if &tag[..3] != b"OBJ" || !(b'1'..=b'3').contains(&tag[3]) {
        return Err(ParseError {
            offset: start,
            message: "OBJ ヘッダが見つかりません".into(),
        });
    }
    let tag_version = tag[3] - b'0';
    let kind = r.u32()?;
    let link = r.u32()?;
    let rect = [r.f64()?, r.f64()?, r.f64()?, r.f64()?];
    let body = match kind {
        KIND_PICTURE => Body::Picture(read_picture(r, tag_version)?),
        KIND_CAPTION => Body::Caption(read_caption(r, tag_version)?),
        KIND_CM_VALUE => Body::CmValue(read_cm_value(r, tag_version)?),
        KIND_DRAW_OBJECT | KIND_DRAFT | KIND_RECT | KIND_PATTERN | KIND_POLY => {
            // 長さを知るためだけに本体の並び（0x426840 / 0x41d320 / 0x421640 / 0x421870 / 0x421b00）を辿る
            let body_start = r.pos;
            skip_passthrough_body(r, kind)?;
            Body::Passthrough(r.data[body_start..r.pos].to_vec())
        }
        other => {
            return Err(ParseError {
                offset: start + 4,
                message: format!("未知の種類 {other}"),
            });
        }
    };
    let trailer = if tag_version >= 3 {
        Some(r.u32()?)
    } else {
        None
    };
    Ok(Object {
        tag_version,
        kind,
        link,
        rect,
        body,
        trailer,
    })
}

fn write_object(w: &mut Writer, o: &Object) {
    w.bytes(b"OBJ");
    w.u8(b'0' + o.tag_version.clamp(1, 3));
    w.u32(o.kind);
    w.u32(o.link);
    for v in o.rect {
        w.f64(v);
    }
    match &o.body {
        Body::Picture(p) => write_picture(w, p),
        Body::Caption(c) => write_caption(w, c),
        Body::CmValue(c) => write_cm_value(w, c),
        Body::Passthrough(raw) => w.bytes(raw),
    }
    if o.tag_version >= 3 {
        w.u32(o.trailer.unwrap_or(0));
    }
}

fn read_picture(r: &mut Reader, tag_version: u8) -> Result<PictureBody> {
    Ok(PictureBody {
        crop: r.array()?,
        b8: r.u32()?,
        bc: r.u32()?,
        guid: r.cstring()?,
        c4: r.cstring()?,
        path: r.cstring()?,
        cc: r.u32()?,
        d0: r.u32()?,
        d4: r.u32()?,
        d8: r.u32()?,
        dc: r.cstring()?,
        e0: r.u32()?,
        e4: r.cstring()?,
        e8: r.u32()?,
        // 0x42c060: 版（タグの数字）が 1 より大きいときだけ +0xec を読む
        ec: if tag_version > 1 {
            Some(r.u32()?)
        } else {
            None
        },
    })
}

fn write_picture(w: &mut Writer, p: &PictureBody) {
    w.bytes(&p.crop);
    w.u32(p.b8);
    w.u32(p.bc);
    w.cstring(&p.guid);
    w.cstring(&p.c4);
    w.cstring(&p.path);
    w.u32(p.cc);
    w.u32(p.d0);
    w.u32(p.d4);
    w.u32(p.d8);
    w.cstring(&p.dc);
    w.u32(p.e0);
    w.cstring(&p.e4);
    w.u32(p.e8);
    if let Some(v) = p.ec {
        w.u32(v);
    }
}

fn read_caption(r: &mut Reader, tag_version: u8) -> Result<CaptionBody> {
    let text = r.cstring()?;
    let face = r.array()?;
    let fmt = r.array()?;
    let value = r.cstring()?;
    let field = r.cstring()?;
    let t108 = r.i32()?;
    let is_value = r.i32()?;
    let border = r.i32()?;
    let border_inset = r.i32()?;
    // 0x408610: +0x114 が 0..2 で、版が 1 より大きいときだけ続きの 2 つを読む
    let border_style = if (0..=2).contains(&border_inset) && tag_version > 1 {
        Some((r.u32()?, r.i32()?))
    } else {
        None
    };
    Ok(CaptionBody {
        text,
        face,
        fmt,
        value,
        field,
        t108,
        is_value,
        border,
        border_inset,
        border_style,
    })
}

fn write_caption(w: &mut Writer, c: &CaptionBody) {
    w.cstring(&c.text);
    w.bytes(&c.face);
    w.bytes(&c.fmt);
    w.cstring(&c.value);
    w.cstring(&c.field);
    w.i32(c.t108);
    w.i32(c.is_value);
    w.i32(c.border);
    w.i32(c.border_inset);
    if let Some((color, width)) = c.border_style {
        w.u32(color);
        w.i32(width);
    }
}

fn read_cm_value(r: &mut Reader, tag_version: u8) -> Result<CmValueBody> {
    let text = r.cstring()?;
    let c0 = r.u32()?;
    let c4 = r.u32()?;
    let x138 = r.u32()?;
    let x13c = r.u32()?;
    let x148 = r.u32()?;
    let face = r.array()?;
    let fmt = r.array()?;
    let x140 = r.u32()?;
    let x154 = r.u32()?;
    let x150 = r.u32()?;
    let x14c = r.u32()?;
    let x144 = r.u32()?;
    let ext = if tag_version > 1 {
        let a = r.u32()?;
        let b = r.u32()?;
        let n = r.u32()? as usize;
        if n > 10_000 {
            return Err(r.err("施工管理値の行数が大きすぎます"));
        }
        let mut rows = Vec::with_capacity(n);
        for _ in 0..n {
            rows.push(r.array()?);
        }
        Some((a, b, rows))
    } else {
        None
    };
    Ok(CmValueBody {
        text,
        c0,
        c4,
        x138,
        x13c,
        x148,
        face,
        fmt,
        x140,
        x154,
        x150,
        x14c,
        x144,
        ext,
    })
}

fn write_cm_value(w: &mut Writer, c: &CmValueBody) {
    w.cstring(&c.text);
    for v in [c.c0, c.c4, c.x138, c.x13c, c.x148] {
        w.u32(v);
    }
    w.bytes(&c.face);
    w.bytes(&c.fmt);
    for v in [c.x140, c.x154, c.x150, c.x14c, c.x144] {
        w.u32(v);
    }
    if let Some((a, b, rows)) = &c.ext {
        w.u32(*a);
        w.u32(*b);
        w.u32(rows.len() as u32);
        for row in rows {
            w.bytes(row);
        }
    }
}

fn read_base(r: &mut Reader) -> Result<DrawBase> {
    Ok(DrawBase {
        raw_90: r.u32()?,
        raw_94: r.u32()?,
        raw_98: r.u8()?,
        pen: r.u32()?,
        pen_style: r.u32()?,
        pen_width: r.i32()?,
        pen_width_y: r.i32()?,
        pen_color: r.u32()?,
        brush: r.u32()?,
        brush_style: r.u32()?,
        brush_color: r.u32()?,
        brush_hatch: r.u32()?,
    })
}

/// 透過保持する本体を読み飛ばす（中身は保持するだけで解釈しない）
fn skip_passthrough_body(r: &mut Reader, kind: u32) -> Result<()> {
    match kind {
        // CDrawObject: u16 ×3, 8 バイト, u32 ×2（0x426840）
        KIND_DRAW_OBJECT => {
            r.bytes(22)?;
        }
        // CDraftObject: 32 バイト, u32 ×2, 文字列 ×2, u32 ×4, 文字列, u32, 文字列, u32（0x41d320）
        KIND_DRAFT => {
            r.bytes(32 + 8)?;
            r.cstring()?;
            r.cstring()?;
            r.bytes(16)?;
            r.cstring()?;
            r.bytes(4)?;
            r.cstring()?;
            r.bytes(4)?;
        }
        // CDrawRect: 共通 45 バイト + u32 ×5 + double ×2（0x421640）。CDrawPattern は + u32（0x421870）
        KIND_RECT | KIND_PATTERN => {
            read_base(r)?;
            r.bytes(20 + 16)?;
            if kind == KIND_PATTERN {
                r.bytes(4)?;
            }
        }
        // CDrawPoly: 共通 45 バイト + 点の数 + 点 ×（double ×2）（0x421b00）
        _ => {
            read_base(r)?;
            let n = r.u32()? as usize;
            if n > 100_000 {
                return Err(r.err("点の数が大きすぎます"));
            }
            r.bytes(n * 16)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 色（COLORREF = 0x00BBGGRR）

pub fn colorref_rgb(c: u32) -> [u8; 3] {
    [
        (c & 0xff) as u8,
        ((c >> 8) & 0xff) as u8,
        ((c >> 16) & 0xff) as u8,
    ]
}

pub fn rgb_colorref(rgb: [u8; 3]) -> u32 {
    rgb[0] as u32 | (rgb[1] as u32) << 8 | (rgb[2] as u32) << 16
}

/// 文字項目の枠線の太さ（0x580578..: 0.1 / 0.3 / 0.7 mm）
pub const BORDER_WIDTHS_MM: [f64; 3] = [0.1, 0.3, 0.7];

#[cfg(test)]
mod tests {
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
}
