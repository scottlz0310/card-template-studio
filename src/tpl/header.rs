//! TMP1 ヘッダ。

use super::MM_PER_INCH;

/// 'TMP1' ヘッダ（20 バイト, 0x445ca0 が組み立て, 0x444f00 が検査）
#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    pub version: u16,
    /// 1〜4。カード内オブジェクトの最大版 10→2, 11→3, 12→4（0x445000）
    pub sub_version: u16,
    /// GetEnterKbn の戻り値。登録した工事の適用区分（要領）から決まる入力区分で、本体が「テンプレートとして
    /// 登録」するたびに、その工事の値で書き直す（国交省 一般土木 H16 = 8、H20 = 14、H22・H28・港湾局 R04 = 15、
    /// 電気通信設備 H22 = 16、営繕工事 H24 = 17。実機で確認、#18）。別の値のテンプレートも一覧に出て適用できる
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
