pub mod dynamic;
pub mod json;
pub mod msgpack;
pub mod rune;

use anyhow::{Result, anyhow};
pub fn skip_white(buf: &[u8]) -> Result<usize> {
    let mut pos = 0usize;
    while pos < buf.len() && (buf[pos] == b' ' || buf[pos] == b'\r' || buf[pos] == b'\t' || buf[pos] == b'\n') {
        pos += 1;
    }
    if pos < buf.len() {
        Ok(pos)
    } else {
        Err(anyhow!("no more data"))
    }
}

pub struct ZOnce {
    first: Option<&'static str>,
    other: &'static str,
}

impl ZOnce {
    pub fn new(first: &'static str, other: &'static str) -> Self {
        Self { first: Some(first), other }
    }
    pub fn take(&mut self) -> &'static str {
        self.first.take().unwrap_or(self.other)
    }
}

#[macro_export]
macro_rules! assert_ok {
    ( $x: expr, $ok: expr) => {
        if $x {
            return Ok($ok);
        }
    };
}

#[macro_export]
macro_rules! assert_err {
    ( $x: expr, $err: expr) => {
        if $x {
            return Err($err);
        }
    };
}

#[macro_export]
macro_rules! dynamic {
    ($($k:expr => $v:expr), *) => {{
        let mut obj = std::collections::BTreeMap::new();
        $( let _ = obj.insert(smol_str::SmolStr::from($k), Dynamic::from($v)); )*
        Dynamic::from_map(obj)
    }};
}