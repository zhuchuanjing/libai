use anyhow::{anyhow, Result};

const TOKEN: &[u8] = b"01234567890.-+truefalsenull"; //合法的数字和其他 json token
pub trait FromJson: Sized {
    fn from_json(buf: &[u8]) -> Result<(Self, usize)>;
    fn get_token(buf: &[u8]) -> Result<(&str, usize)> {
        let mut pos = 0usize;
        while pos < buf.len() && TOKEN.contains(&buf[pos]) {
            pos += 1;
        }
        Ok((std::str::from_utf8(&buf[..pos])?, pos))
    }
    
    fn get_string(buf: &[u8]) -> Result<(String, usize)> {
        let mut pos = 1usize;
        let mut vec = Vec::new();
        while pos < buf.len() && buf[pos] != b'"' {
            if buf[pos] == b'\\' {
                pos += 1;
                if pos == buf.len() {
                    return Err(anyhow!("uncomplete string"));
                }
                match buf[pos] {
                    b'\\'=> vec.push(b'\\'),
                    b'"'=> vec.push(b'\"'),
                    b'r'=> vec.push(b'\r'),
                    b'n'=> vec.push(b'\n'),
                    b't'=> vec.push(b'\t'),
                    b'u'=> {
                        let unicode_str = unsafe { std::str::from_utf8_unchecked(&buf[(pos + 1)..(pos + 5)]) };
                        if let Ok(unicode) = u32::from_str_radix(unicode_str, 16) {
                            if unicode < 0x80 {
                                vec.push(unicode as u8);
                            } else {
                                if let Some(unicode_char) = char::from_u32(unicode) {
                                    unicode_char.encode_utf8(&mut vec);
                                }
                            }
                        }
                        pos += 4;
                    },
                    _=> {
                        return Err(anyhow!("unknow escape {}", buf[pos]));
                    }
                }
            } else {
                vec.push(buf[pos]);
            }
            pos += 1;
        }
        if pos < buf.len() {
            pos += 1;
        }
        Ok(String::from_utf8(vec).map(|s| (s, pos))?)
    }
}

use super::dynamic::Dynamic;
use super::skip_white;
use std::collections::BTreeMap;
use smol_str::SmolStr;
use super::assert_err;

impl FromJson for Dynamic {
    fn from_json(buf: &[u8])-> Result<(Self, usize)> {
        let mut pos = skip_white(buf)?;
        if buf[pos] == b'[' {           //是一个 vec
            pos += 1;
            pos += skip_white(&buf[pos..])?;
            let mut vec = Vec::<Self>::new();
            while buf[pos] != b']' {
                let (item, size) = Self::from_json(&buf[pos..])?;
                vec.push(item);
                pos += size;
                pos += skip_white(&buf[pos..])?;
                if buf[pos] == b',' {
                    pos += 1;
                    pos += skip_white(&buf[pos..])?;
                }
            };
            Ok((Dynamic::from_vec(vec), pos + 1))
        } else if buf[pos] == b'{' {           //是一个 object
            pos += 1;
            pos += skip_white(&buf[pos..])?;
            let mut obj = BTreeMap::new();
            while buf[pos] != b'}' {
                assert_err!(buf[pos] != b'"', anyhow!("need a string key"));
                let (key, size) = Self::get_string(&buf[pos..])?;
                pos += size;
                pos += skip_white(&buf[pos..])?;
                assert_err!(buf[pos] != b':', anyhow!("need a :"));
                pos += 1;
                pos += skip_white(&buf[pos..])?;
                let (item, size) = Self::from_json(&buf[pos..])?;
                obj.insert(SmolStr::from(key), item);
                pos += size;
                pos += skip_white(&buf[pos..])?;
                if buf[pos] == b',' {
                    pos += 1;
                    pos += skip_white(&buf[pos..])?;
                }
            }
            Ok((Dynamic::from_map(obj), pos + 1))
        } else if buf[pos] == b'"' {
            let (s, size) = Self::get_string(&buf[pos..])?;
            Ok((s.into(), size))
        } else {
            let (token, size) = Self::get_token(&buf[pos..])?;
            if token == "true" {
                Ok((Dynamic::from(true), size))
            } else if token == "false" {
                Ok((Dynamic::from(false), size))
            } else if token == "null" {
                Ok((Dynamic::Null, size))
            } else if token.contains('.') {
                let v = token.parse::<f64>()?;
                Ok((Dynamic::from(v), size))
            } else {
                let v = token.parse::<i64>()?;
                Ok((Dynamic::from(v), size))
            }
        }
    }
}

pub trait ToJson {
    fn to_json(&self, buf: &mut String);
}

impl ToJson for &str {
    fn to_json(&self, buf: &mut String) {
        let mut formatted = self.as_bytes().iter().fold(vec![b'\"'], |mut vec, ch| {
            match ch {
                b'\"'=> { vec.extend_from_slice(&[0x5c, 0x22]); vec }
                b'\\'=> { vec.extend_from_slice(&[0x5c, 0x5c]); vec }
                b'\n'=> { vec.extend_from_slice(&[0x5c, 0x6e]); vec }
                b'\r'=> { vec.extend_from_slice(&[0x5c, 0x72]); vec }
                b'\t'=> { vec.extend_from_slice(&[0x5c, 0x74]); vec }
                _=> { vec.push(*ch); vec }
            }
        });
        formatted.push(b'\"');
        buf.push_str(unsafe{ std::str::from_utf8_unchecked(&formatted) });
    }
}

impl ToJson for i64 {
    fn to_json(&self, buf: &mut String) {
        buf.push_str(&self.to_string());
    }
}

impl ToJson for Dynamic {
    fn to_json(&self, buf: &mut String) {
        match self {
            Self::Bool(b) => if *b { buf.push_str("true") } else { buf.push_str("false") }
            Self::Double(f) => buf.push_str(&f.to_string()),
            Self::Int(i) => i.to_json(buf),
            Self::Null => buf.push_str("null"),
            Self::String(s) => s.as_str().to_json(buf),
            Self::Vec(a) => {
                buf.push('[');
                let mut once = super::ZOnce::new("", ",\n");
                a.read().unwrap().iter().for_each(|item| {
                    buf.push_str(once.take());
                    item.to_json(buf);
                });
                buf.push(']');
            }
            Self::Map(m) => {
                buf.push('{');
                let mut once = super::ZOnce::new("", ",\n");
                m.read().unwrap().iter().for_each(|(k, v)| {
                    buf.push_str(once.take());
                    k.as_str().to_json(buf);
                    buf.push_str(": ");
                    v.to_json(buf);
                });
                buf.push('}');
            },
            _=> buf.push_str("null")
        }
    }
}