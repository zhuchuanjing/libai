use super::dynamic::Dynamic;
use rune::{ToValue, Value};

//rune Value 和 Dynamic 类型的相互转换
use std::sync::{Arc, RwLock};
use smol_str::SmolStr;
use std::collections::BTreeMap;

impl From<&Value> for Dynamic {
    fn from(value: &Value)-> Self {
        match value {
            Value::EmptyTuple | Value::EmptyStruct(_)=> Self::Null,
            Value::Bool(b)=> Self::Bool(*b),
            Value::Byte(b)=> Self::Byte(*b),
            Value::Integer(i)=> Self::Int(*i),
            Value::Float(f)=> Self::Double(*f),
            Value::String(s)=> Self::String(Arc::new(SmolStr::new(s.borrow_ref().unwrap().as_str()))),
            Value::Vec(v)=> {
                let array: Vec<Dynamic> = v.borrow_ref().unwrap().iter().map(|v| Self::from(v) ).collect();
                Self::Array(Arc::new(RwLock::new(array)))
            },
            Value::Object(o)=> {
                let mut objec = BTreeMap::new();
                o.borrow_ref().unwrap().iter().for_each(|(k, v)| {
                    objec.insert(SmolStr::new(k.as_str()), Self::from(v) );
                });
                Self::Object(Arc::new(RwLock::new(objec))) 
            },
            Value::Bytes(b)=> Self::Bytes(Arc::new(b.borrow_ref().unwrap().as_slice().to_vec())),
            v=> {
                println!("{:?}", v);
                Self::Null
            }
        }
    }
}

pub fn str_to_rune(s: &str)-> rune::alloc::String {
    let mut v = rune::alloc::Vec::try_with_capacity(s.len()).unwrap();
    v.try_extend_from_slice(s.as_bytes()).unwrap();
    unsafe {rune::alloc::String::from_utf8_unchecked(v) }
}

pub fn bytes_to_rune(bytes: &[u8])-> rune::alloc::String {
    unsafe {rune::alloc::String::from_utf8_unchecked(rune::alloc::Vec::try_from(bytes).unwrap()) }
}

impl From<&Dynamic> for Value {
    fn from(d: &Dynamic) -> Self {
        match d {
            Dynamic::Null=> Value::EmptyTuple,
            Dynamic::Bool(b)=> Value::Bool(*b),
            Dynamic::Byte(b)=> Value::Byte(*b),
            Dynamic::Int(i)=> Value::Integer(*i),
            Dynamic::UInt(u)=> Value::Integer(*u as i64),
            Dynamic::Float(f)=> Value::Float(*f as f64),
            Dynamic::Double(f)=> Value::Float(*f),
            Dynamic::String(s)=> str_to_rune(s.as_str()).to_value().unwrap(),
            Dynamic::Array(array)=> {
                let array = array.read().unwrap().iter().fold(rune::alloc::Vec::default(), |mut v, a| {
                    v.try_push(Self::from(a)).unwrap();
                    v
                });
                array.to_value().unwrap()
            },
            Dynamic::Object(object)=> {
                let object = object.read().unwrap().iter().fold(rune::runtime::Object::default(), |mut obj, (k, v)| {
                    obj.insert(str_to_rune(k.as_str()), Self::from(v)).unwrap();
                    obj
                });
                object.to_value().unwrap()
            },
            Dynamic::Bytes(b)=> {
                b.as_slice().to_vec().to_value().unwrap()
            },
        }
    }
}

use super::msgpack::MsgPack;
use byteorder::{BigEndian, WriteBytesExt};

impl MsgPack for Value {
    fn encode(&self, buf: &mut Vec<u8>) {
        match self {
            Value::EmptyTuple | Value::EmptyStruct(_) => buf.push(0xc0),
            Value::Bool(v) => buf.push(if *v { 0xc3 } else { 0xc2 }),
            Value::Byte(b) => {
                buf.push(0xcc);
                buf.push(*b as u8);
            }
            Value::Integer(v) => v.encode(buf),
            Value::Float(v) => {
                buf.push(0xcb);
                let int_value = unsafe { std::mem::transmute::<f64, u64>(*v) };
                buf.write_u64::<BigEndian>(int_value).unwrap();
            }
            Value::String(s) => s.borrow_ref().unwrap().as_str().encode(buf),
            Value::Bytes(v) => {
                v.borrow_ref()
                    .map(|raw| {
                        let length = raw.len();
                        if length < 0x100 {
                            buf.push(0xc4);
                            buf.push(length as u8);
                        } else if length < 0x10000 {
                            buf.push(0xc5);
                            buf.write_u16::<BigEndian>(length as u16).unwrap();
                        } else {
                            buf.push(0xc6);
                            buf.write_u32::<BigEndian>(length as u32).unwrap();
                        }
                        buf.extend_from_slice(raw.as_slice());
                    })
                    .unwrap();
            }
            Value::Vec(v) => {
                v.borrow_ref()
                    .map(|raw| {
                        let length = raw.len();
                        if length < 0x10 {
                            buf.push(0x90 | length as u8);
                        } else if length < 0x10000 {
                            buf.push(0xdc);
                            buf.write_u16::<BigEndian>(length as u16).unwrap();
                        } else {
                            buf.push(0xdd);
                            buf.write_u32::<BigEndian>(length as u32).unwrap();
                        }
                        raw.iter().for_each(|item| item.encode(buf));
                    })
                    .unwrap();
            }
            Value::Object(v) => {
                v.borrow_ref()
                    .map(|raw| {
                        let length = raw.len();
                        if length < 16 {
                            buf.push(0x80 | length as u8);
                        } else if length <= 0x10000 {
                            buf.push(0xde);
                            buf.write_u16::<BigEndian>(length as u16).unwrap();
                        } else {
                            buf.push(0xdf);
                            buf.write_u32::<BigEndian>(length as u32).unwrap();
                        }
                        raw.iter().for_each(|(k, v)| {
                            k.as_str().encode(buf);
                            v.encode(buf);
                        });
                    })
                    .unwrap();
            }
            _ => {}
        }
    }
}

#[macro_export]
macro_rules! object {
    ($($k:expr => $v:expr), *) => {{
        let mut obj = rune::runtime::Object::default();
        $( let _ = obj.insert(rune::alloc::String::try_from($k).unwrap(), rune::runtime::Value::try_from($v)?); )*
        obj.try_into().unwrap()
    }};
}

use super::json::{FromJson, ToJson};
use super::skip_white;
impl FromJson for Value {
    fn from_json(buf: &[u8])-> Result<(Self, usize)> {
        let mut pos = skip_white(buf)?;
        if buf[pos] == b'[' {           //是一个 vec
            pos += 1;
            pos += skip_white(&buf[pos..])?;
            let mut vec = rune::alloc::Vec::<Self>::new();
            while buf[pos] != b']' {
                let (item, size) = Self::from_json(&buf[pos..])?;
                vec.try_push(item)?;
                pos += size;
                pos += skip_white(&buf[pos..])?;
                if buf[pos] == b',' {
                    pos += 1;
                    pos += skip_white(&buf[pos..])?;
                }
            };
            Ok((vec.to_value().unwrap(), pos + 1))
        } else if buf[pos] == b'{' {           //是一个 object
            pos += 1;
            pos += skip_white(&buf[pos..])?;
            let mut obj = rune::runtime::Object::new();
            while buf[pos] != b'}' {
                assert_err!(buf[pos] != b'"', anyhow!("need a string key"));
                let (key, size) = Self::get_string(&buf[pos..])?;
                pos += size;
                pos += skip_white(&buf[pos..])?;
                assert_err!(buf[pos] != b':', anyhow!("need a :"));
                pos += 1;
                pos += skip_white(&buf[pos..])?;
                let (item, size) = Self::from_json(&buf[pos..])?;
                obj.insert(str_to_rune(key.as_str()), item)?;
                pos += size;
                pos += skip_white(&buf[pos..])?;
                if buf[pos] == b',' {
                    pos += 1;
                    pos += skip_white(&buf[pos..])?;
                }
            }
            Ok((obj.to_value().unwrap(), pos + 1))
        } else if buf[pos] == b'"' {
            let (s, size) = Self::get_string(&buf[pos..])?;
            Ok((str_to_rune(s.as_str()).to_value().into_result()?, size))
        } else {
            let (token, size) = Self::get_token(&buf[pos..])?;
            if token == "true" {
                Ok((Value::from(true), size))
            } else if token == "false" {
                Ok((Value::from(false), size))
            } else if token == "null" {
                Ok((Value::from(()), size))
            } else if token.contains('.') {
                let v = token.parse::<f64>()?;
                Ok((Value::from(v), size))
            } else {
                let v = token.parse::<i64>()?;
                Ok((Value::from(v), size))
            }
        }
    }
}

use rune::runtime::Object;
impl ToJson for Object {
    fn to_json(&self, buf: &mut String) {
        buf.push('{');
        let mut once = super::ZOnce::new("", ",\n");
        self.iter().for_each(|item| {
            buf.push_str(once.take());
            item.0.as_str().to_json(buf);
            buf.push_str(": ");
            item.1.to_json(buf);
        });
        buf.push('}');
    }
}

impl ToJson for Value {
    fn to_json(&self, buf: &mut String) {
        match self {
            Self::Bool(b) => if *b { buf.push_str("true") } else { buf.push_str("false") }
            Self::Float(f) => buf.push_str(&f.to_string()),
            Self::Integer(i) => i.to_json(buf),
            Self::EmptyTuple | Value::EmptyStruct(_) => buf.push_str("null"),
            Self::String(s) =>s.borrow_ref().unwrap().as_str().to_json(buf),
            Self::Vec(v) => {
                buf.push('[');
                let mut once = super::ZOnce::new("", ",\n");
                v.borrow_ref().map(|raw| raw.iter().for_each(|item| {
                    buf.push_str(once.take());
                    item.to_json(buf);
                })).unwrap();
                buf.push(']');
            }
            Self::Object(m) => m.borrow_ref().map(|o| o.to_json(buf)).unwrap(),
            _ => {
                buf.push_str("null");
            }
        }
    }
}

use super::msgpack::MsgUnpack;
use super::{assert_err, assert_ok};
use anyhow::{Result, anyhow};

fn slice_to_string(slice: &[u8]) -> Result<Value> {
    let buf: rune::alloc::Vec<u8> = slice.try_into()?;
    let s = rune::alloc::String::from_utf8(buf)?;
    Ok(s.try_into()?)
}

fn slice_to_bytes(slice: &[u8]) -> Result<Value> {
    Ok(Value::try_from(rune::runtime::Bytes::from_slice(slice)?)?)
}

fn vec_to_object(kvs: Vec<Value>) -> Result<Value> {
    let mut obj = rune::runtime::Object::with_capacity(kvs.len())?;
    let mut k: Option<Value> = None;
    for kv in kvs {
        if let Some(k) = k.take() {
            obj.insert(k.into_string().unwrap().take()?, kv)?;
        } else {
            k = Some(kv);
        }
    }
    Ok(obj.try_into()?)
}

use super::msgpack::{read_8, read_16, read_32, read_64};
impl MsgUnpack for Value {
    fn decode(buf: &[u8]) -> Result<(Self, usize)> {
        assert_err!(buf.len() < 1, anyhow!("no data"));
        let first_byte = buf[0];
        assert_ok!(first_byte <= 0x7f, (Value::from(first_byte as i64), 1));
        assert_ok!(first_byte >= 0xe0, (Value::from(first_byte as i64 - 256), 1));
        if first_byte >= 0x80 && first_byte <= 0x8f {
            let len = (first_byte & 0x0f) as usize;
            let (value, size) = Self::decode_array(&buf[1..], len * 2)?;
            return vec_to_object(value).map(|r| (r, 1 + size));
        }
        if first_byte >= 0x90 && first_byte <= 0x9f {
            let len = (first_byte & 0x0f) as usize;
            let (value, size) = Self::decode_array(&buf[1..], len)?;
            return Ok((value.to_value().unwrap(), 1 + size));
        }

        if first_byte >= 0xa0 && first_byte <= 0xbf {
            let len = (first_byte & 0x1f) as usize;
            assert_err!(buf.len() < 1 + len, anyhow!("no data"));
            return slice_to_string(&buf[1..1 + len]).map(|r| (r, 1 + len));
        }

        assert_ok!(first_byte == 0xc0, (().into(), 1));
        assert_err!(first_byte == 0xc1, anyhow!("0xc1 never used"));
        assert_ok!(first_byte == 0xc2, (false.into(), 1));
        assert_ok!(first_byte == 0xc3, (true.into(), 1));

        if first_byte == 0xc4 {
            assert_err!(buf.len() < 2, anyhow!("no data"));
            let len = read_8(&buf[1..]) as usize;
            assert_err!(buf.len() < 2 + len, anyhow!("no data"));
            return slice_to_bytes(&buf[2..2 + len]).map(|r| (r, 2 + len));
        }

        if first_byte == 0xc5 {
            assert_err!(buf.len() < 3, anyhow!("no data"));
            let len = read_16(&buf[1..]) as usize;
            assert_err!(buf.len() < 2 + len, anyhow!("no data"));
            return slice_to_bytes(&buf[3..3 + len]).map(|r| (r, 3 + len));
        }

        if first_byte == 0xc6 {
            assert_err!(buf.len() < 5, anyhow!("no data"));
            let len = read_32(&buf[1..]) as usize;
            assert_err!(buf.len() < 5 + len, anyhow!("no data"));
            return slice_to_bytes(&buf[5..5 + len]).map(|r| (r, 5 + len));
        }

        if first_byte == 0xc7 {
            assert_err!(buf.len() < 3, anyhow!("no data"));
            let len = read_8(&buf[1..]) as usize;
            let _type_id = unsafe { std::mem::transmute::<u8, i8>(buf[2]) };
            assert_err!(buf.len() < 3 + len, anyhow!("no data"));
            //let _value = raw[3..3 + len].to_vec();
            return Ok((().into(), 3 + len)); //暂时没实现
        }

        if first_byte == 0xc8 {
            assert_err!(buf.len() < 4, anyhow!("no data"));
            let len = read_16(&buf[1..]) as usize;
            let _type_id = unsafe { std::mem::transmute::<u8, i8>(buf[3]) };
            assert_err!(buf.len() < 4 + len, anyhow!("no data"));
            //let _value = raw[4..4 + len].to_vec();
            return Ok((().into(), 4 + len)); //暂时没实现
        }

        if first_byte == 0xc9 {
            assert_err!(buf.len() < 6, anyhow!("no data"));
            let len = read_32(&buf[1..]) as usize;
            let _type_id = unsafe { std::mem::transmute::<u8, i8>(buf[5]) };
            assert_err!(buf.len() < 6 + len, anyhow!("no data"));
            //let _value = raw[6..6 + len].to_vec();
            return Ok((().into(), 6 + len)); //暂时没实现
        }

        if first_byte == 0xca {
            assert_err!(buf.len() < 5, anyhow!("no data"));
            let raw_value = read_32(&buf[1..]) as u32;
            let value = unsafe { std::mem::transmute::<u32, f32>(raw_value) };
            return Ok((Value::from(value as f64), 5));
        }

        if first_byte == 0xcb {
            assert_err!(buf.len() < 9, anyhow!("no data"));
            let raw_value = read_64(&buf[1..]);
            let value = unsafe { std::mem::transmute::<u64, f64>(raw_value) };
            return Ok((Value::from(value), 9));
        }

        if first_byte == 0xcc {
            assert_err!(buf.len() < 2, anyhow!("no data"));
            let value = read_8(&buf[1..]);
            return Ok((Value::from(value as i64), 2));
        }

        if first_byte == 0xcd {
            assert_err!(buf.len() < 3, anyhow!("no data"));
            let value = read_16(&buf[1..]);
            return Ok((Value::from(value as i64), 3));
        }

        if first_byte == 0xce {
            assert_err!(buf.len() < 5, anyhow!("no data"));
            let value = read_32(&buf[1..]);
            return Ok((Value::from(value as i64), 5));
        }

        if first_byte == 0xcf {
            assert_err!(buf.len() < 9, anyhow!("no data"));
            let value = read_64(&buf[1..]);
            return Ok((Value::from(value as i64), 9));
        }

        if first_byte == 0xd0 {
            assert_err!(buf.len() < 2, anyhow!("no data"));
            let raw_value = read_8(&buf[1..]) as i8;
            let value = raw_value as i64; //unsafe { std::mem::transmute::<u64, i64>(raw_value) };
            return Ok((Value::from(value), 2));
        }

        if first_byte == 0xd1 {
            assert_err!(buf.len() < 3, anyhow!("no data"));
            let raw_value = read_16(&buf[1..]);
            let value = unsafe { std::mem::transmute::<u16, i16>(raw_value) } as i64;
            return Ok((Value::from(value), 3));
        }

        if first_byte == 0xd2 {
            assert_err!(buf.len() < 5, anyhow!("no data"));
            let raw_value = read_32(&buf[1..]);
            let value = unsafe { std::mem::transmute::<u32, i32>(raw_value) } as i64;
            return Ok((Value::from(value), 5));
        }

        if first_byte == 0xd3 {
            assert_err!(buf.len() < 9, anyhow!("no data"));
            let raw_value = read_64(&buf[1..]);
            let value = unsafe { std::mem::transmute::<u64, i64>(raw_value) };
            return Ok((Value::from(value), 9));
        }

        if first_byte == 0xd4 {
            assert_err!(buf.len() < 3, anyhow!("no data"));
            let _type_id = unsafe { std::mem::transmute::<u8, i8>(buf[1]) };
            //let _value = raw[2..3].to_vec();
            return Ok((().into(), 3));
        }

        if first_byte == 0xd5 {
            assert_err!(buf.len() < 4, anyhow!("no data"));
            let _type_id = unsafe { std::mem::transmute::<u8, i8>(buf[1]) };
            //let _value = raw[2..4].to_vec();
            return Ok((().into(), 4));
        }

        if first_byte == 0xd6 {
            assert_err!(buf.len() < 6, anyhow!("no data"));
            let _type_id = unsafe { std::mem::transmute::<u8, i8>(buf[1]) };
            //let _value = raw[2..6].to_vec();
            return Ok((().into(), 6));
        }

        if first_byte == 0xd7 {
            assert_err!(buf.len() < 10, anyhow!("no data"));
            let _type_id = unsafe { std::mem::transmute::<u8, i8>(buf[1]) };
            //let _value = raw[2..10].to_vec();
            return Ok((().into(), 10));
        }

        if first_byte == 0xd8 {
            assert_err!(buf.len() < 18, anyhow!("no data"));
            let _type_id = unsafe { std::mem::transmute::<u8, i8>(buf[1]) };
            //let _value = raw[2..18].to_vec();
            return Ok((().into(), 18));
        }

        if first_byte == 0xd9 {
            assert_err!(buf.len() < 2, anyhow!("no data"));
            let len = read_8(&buf[1..]) as usize;
            assert_err!(buf.len() < 2 + len, anyhow!("no data"));
            return slice_to_string(&buf[2..2 + len]).map(|r| (r, 2 + len));
        }

        if first_byte == 0xda {
            assert_err!(buf.len() < 3, anyhow!("no data"));
            let len = read_16(&buf[1..]) as usize;
            assert_err!(buf.len() < 3 + len, anyhow!("no data"));
            return slice_to_string(&buf[3..3 + len]).map(|r| (r, 3 + len));
        }

        if first_byte == 0xdb {
            assert_err!(buf.len() < 5, anyhow!("no data"));
            let len = read_32(&buf[1..]) as usize;
            assert_err!(buf.len() < 5 + len, anyhow!("no data"));
            return slice_to_string(&buf[5..5 + len]).map(|r| (r, 5 + len));
        }

        if first_byte == 0xdc {
            assert_err!(buf.len() < 3, anyhow!("no data"));
            let len = read_16(&buf[1..]) as usize;
            let (value, size) = Self::decode_array(&buf[3..], len)?;
            return Ok((value.to_value().unwrap(), 3 + size));
        }

        if first_byte == 0xdd {
            assert_err!(buf.len() < 5, anyhow!("no data"));
            let len = read_32(&buf[1..]) as usize;
            let (value, size) = Self::decode_array(&buf[5..], len)?;
            return Ok((value.to_value().unwrap(), 5 + size));
        }

        if first_byte == 0xde {
            assert_err!(buf.len() < 3, anyhow!("no data"));
            let len = read_16(&buf[1..]) as usize;
            let (value, size) = Self::decode_array(&buf[3..], len * 2)?;
            return vec_to_object(value).map(|r| (r, 3 + size));
        }

        if first_byte == 0xdf {
            assert_err!(buf.len() < 5, anyhow!("no data"));
            let len = read_32(&buf[1..]) as usize;
            let (value, size) = Self::decode_array(&buf[5..], len * 2)?;
            return vec_to_object(value).map(|r| (r, 5 + size));
        }
        Err(anyhow!("error code {}", first_byte))
    }
}