//! 読み書きの補助（バイト列の読み手と書き手）。

use super::error::{ParseError, Result};
use super::text::SjisString;

pub(super) struct Reader<'a> {
    pub(super) data: &'a [u8],
    pub(super) pos: usize,
}

impl<'a> Reader<'a> {
    pub(super) fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    pub(super) fn err(&self, message: impl Into<String>) -> ParseError {
        ParseError {
            offset: self.pos,
            message: message.into(),
        }
    }
    pub(super) fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.pos + n > self.data.len() {
            return Err(self.err(format!(
                "{n} バイト読もうとしてファイルの終わりに達しました"
            )));
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    pub(super) fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut a = [0u8; N];
        a.copy_from_slice(self.bytes(N)?);
        Ok(a)
    }
    pub(super) fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }
    pub(super) fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    pub(super) fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    pub(super) fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.array()?))
    }
    pub(super) fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_le_bytes(self.array()?))
    }
    /// CString: 長さ 1 バイト（0xFF のときは続く u16 が長さ）＋ 本体（0x45c000 / 0x45be30）
    pub(super) fn cstring(&mut self) -> Result<SjisString> {
        let mut n = self.u8()? as usize;
        if n == 0xff {
            n = self.u16()? as usize;
            if n == 0xffff {
                return Err(self.err("4 バイト長の文字列には対応していません"));
            }
        }
        Ok(SjisString(self.bytes(n)?.to_vec()))
    }
    pub(super) fn tag(&mut self) -> Result<[u8; 4]> {
        self.array()
    }
}

#[derive(Default)]
pub(super) struct Writer {
    pub(super) out: Vec<u8>,
}

impl Writer {
    pub(super) fn bytes(&mut self, b: &[u8]) {
        self.out.extend_from_slice(b);
    }
    pub(super) fn u8(&mut self, v: u8) {
        self.out.push(v);
    }
    pub(super) fn u16(&mut self, v: u16) {
        self.bytes(&v.to_le_bytes());
    }
    pub(super) fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }
    pub(super) fn i32(&mut self, v: i32) {
        self.bytes(&v.to_le_bytes());
    }
    pub(super) fn f64(&mut self, v: f64) {
        self.bytes(&v.to_le_bytes());
    }
    pub(super) fn cstring(&mut self, s: &SjisString) {
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
