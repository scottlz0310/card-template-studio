//! 本体のエクスポート／インポートの形式（TMPEXP\expinf, TEMP{横}x{縦}_{H|V} フォルダ）。

/// 本体のエクスポートが書く expinf の中身（0x598d94 付近の文字列 "Ver7" + ".10Template"）
pub const EXPINF_TEXT: &[u8] = b"Ver7.10Template\r\n";

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
