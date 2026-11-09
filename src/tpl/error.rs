//! 読み込みの失敗。

use std::fmt;

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

pub(super) type Result<T> = std::result::Result<T, ParseError>;
