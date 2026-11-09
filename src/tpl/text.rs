//! cp932 の文字列。

use encoding_rs::SHIFT_JIS;

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
