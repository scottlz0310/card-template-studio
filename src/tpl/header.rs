//! TMP1 ヘッダ。

use super::MM_PER_INCH;

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
