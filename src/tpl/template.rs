//! テンプレート全体（ヘッダ + カード + オブジェクト）。

use super::codec::{read_object, write_object};
use super::error::{ParseError, Result};
use super::header::Header;
use super::io::{Reader, Writer};
use super::object::Object;

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
