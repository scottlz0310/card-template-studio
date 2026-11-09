//! 文字の書式（44 バイト）と色。

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
