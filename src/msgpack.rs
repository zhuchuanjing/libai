pub trait MsgPack {                                 //编码为 msgpakc 格式的 trait
    fn encode(&self, buf: &mut Vec<u8>);
}

use std::collections::BTreeMap;

use byteorder::{BigEndian, WriteBytesExt};
use smol_str::SmolStr;
impl MsgPack for &str {
    fn encode(&self, buf: &mut Vec<u8>) {
        let length = self.len();
        if length < 0x20 {
            buf.push(0xa0 | length as u8);
        } else if length < 0x100 {
            buf.push(0xd9);
            buf.push(length as u8);
        } else if length < 0x10000 {
            buf.push(0xda);
            buf.write_u16::<BigEndian>(length as u16).unwrap();
        } else {
            buf.push(0xdb);
            buf.write_u32::<BigEndian>(length as u32).unwrap();
        }
        buf.extend_from_slice(self.as_bytes());
    }
}

impl MsgPack for i64 {
    fn encode(&self, buf: &mut Vec<u8>) {
        let value = *self;
        if value >= 0 && value < 128 {
            buf.push(value as u8);
        } else if value < 0 && value > -32 {
            let raw = unsafe { std::mem::transmute::<i8, u8>(value as i8) };
            buf.push(raw);
        } else {
            if value >= -0x80 && value < 0x80 {
                buf.push(0xd0);
                buf.write_i8(value as i8).unwrap();
            } else if value >= -0x8000 && value < 0x8000 {
                buf.push(0xd1);
                buf.write_i16::<BigEndian>(value as i16).unwrap();
            } else if value >= -0x8000_0000 && value < 0x8000_0000 {
                buf.push(0xd2);
                buf.write_i32::<BigEndian>(value as i32).unwrap();
            } else {
                buf.push(0xd3);
                buf.write_i64::<BigEndian>(value).unwrap();
            }
        }
    }
}

use super::dynamic::Dynamic;

impl MsgPack for Dynamic {
    fn encode(&self, buf: &mut Vec<u8>) {
        match self {
            Dynamic::Null=> buf.push(0xc0),
            Dynamic::Bool(b)=> buf.push(if *b { 0xc3 } else { 0xc2 }),
            Dynamic::Byte(b) => {
                buf.push(0xcc);
                buf.push(*b as u8);
            }
            Dynamic::Int(v) => v.encode(buf),
            Dynamic::UInt(v)=> (*v as i64).encode(buf),                 //rune 脚本语言的 Value 不支持 u64 所以我们按照 i64 处理
            Dynamic::Double(v) => {
                buf.push(0xcb);
                let int_value = unsafe { std::mem::transmute::<f64, u64>(*v) };
                buf.write_u64::<BigEndian>(int_value).unwrap();
            }
            Dynamic::Float(v) => {                                      //Value 不支持 f32 按照 f64 处理
                buf.push(0xcb);
                let int_value = unsafe { std::mem::transmute::<f64, u64>(*v as f64) };
                buf.write_u64::<BigEndian>(int_value).unwrap();
            }
            Dynamic::String(s) => s.as_str().encode(buf),
            Dynamic::Bytes(raw) => {
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
            }
            Dynamic::Vec(raw) => {
                let length = raw.read().unwrap().len();
                if length < 0x10 {
                      buf.push(0x90 | length as u8);
                } else if length < 0x10000 {
                    buf.push(0xdc);
                    buf.write_u16::<BigEndian>(length as u16).unwrap();
                } else {
                    buf.push(0xdd);
                    buf.write_u32::<BigEndian>(length as u32).unwrap();
                }
                raw.read().unwrap().iter().for_each(|item| item.encode(buf));
            }
            Dynamic::Map(raw) => {
                let length = raw.read().unwrap().len();
                if length < 16 {
                    buf.push(0x80 | length as u8);
                } else if length <= 0x10000 {
                    buf.push(0xde);
                    buf.write_u16::<BigEndian>(length as u16).unwrap();
                } else {
                    buf.push(0xdf);
                    buf.write_u32::<BigEndian>(length as u32).unwrap();
                }
                raw.read().unwrap().iter().for_each(|(k, v)| {
                    k.as_str().encode(buf);
                    v.encode(buf);
                });
            }
        }
    }
}
use anyhow::{anyhow, Result};

pub trait MsgUnpack: Sized {                        //解码 msgpack 格式的 trait
    fn decode(buf: &[u8]) -> Result<(Self, usize)>;
    fn decode_array(buf: &[u8], length: usize) -> Result<(Vec<Self>, usize)> {
        let mut cursor = 0usize;
        let mut result = Vec::with_capacity(length);
        for _ in 0..length {
            let (value, size) = Self::decode(&buf[cursor..])?;
            result.push(value);
            cursor += size;
        }
        Ok((result, cursor))
    }
}

#[inline]
pub(crate) fn read_8(raw: &[u8]) -> u8 {
    raw[0]
}

#[inline]
pub(crate) fn read_16(raw: &[u8]) -> u16 {
    raw[1] as u16 | (raw[0] as u16) << 8
}

#[inline]
pub(crate) fn read_32(raw: &[u8]) -> u32 {
    raw[3] as u32 | (raw[2] as u32) << 8 | (raw[1] as u32) << 16 | (raw[0] as u32) << 24
}

#[inline]
pub(crate) fn read_64(raw: &[u8]) -> u64 {
    raw[7] as u64 | (raw[6] as u64) << 8 | (raw[5] as u64) << 16 | (raw[4] as u64) << 24 | (raw[3] as u64) << 32 | (raw[2] as u64) << 40 | (raw[1] as u64) << 48 | (raw[0] as u64) << 56
}

fn vec_to_dynamic(kvs: Vec<Dynamic>) -> Result<Dynamic> {
    let mut map: BTreeMap<SmolStr, Dynamic> = BTreeMap::new();
    let mut key: Option<Dynamic> = None;
    for kv in kvs {
        if let Some(k) = key.take() {
            map.insert(k.into_string()?, kv);
        } else {
            key = Some(kv);
        }
    }
    Ok(Dynamic::from_map(map))   
}

use super::{assert_err, assert_ok};

impl MsgUnpack for Dynamic {
    fn decode(buf: &[u8]) -> Result<(Self, usize)> {
        assert_err!(buf.len() < 1, anyhow!("no data"));
        let first_byte = buf[0];
        assert_ok!(first_byte <= 0x7f, (Dynamic::from(first_byte as i64), 1));
        assert_ok!(first_byte >= 0xe0, (Dynamic::from(first_byte as i64 - 256), 1));
        if first_byte >= 0x80 && first_byte <= 0x8f {
            let len = (first_byte & 0x0f) as usize;
            let (value, size) = Self::decode_array(&buf[1..], len * 2)?;
            return vec_to_dynamic(value).map(|r| (r, 1 + size));
        }
        if first_byte >= 0x90 && first_byte <= 0x9f {
            let len = (first_byte & 0x0f) as usize;
            let (value, size) = Self::decode_array(&buf[1..], len)?;
            return Ok((Dynamic::from_vec(value), 1 + size));
        }

        if first_byte >= 0xa0 && first_byte <= 0xbf {
            let len = (first_byte & 0x1f) as usize;
            assert_err!(buf.len() < 1 + len, anyhow!("no data"));
            return Ok(Dynamic::try_from(&buf[1..1 + len]).map(|r| (r, 1 + len))?);
        }

        assert_ok!(first_byte == 0xc0, (Dynamic::Null, 1));
        assert_err!(first_byte == 0xc1, anyhow!("0xc1 never used"));
        assert_ok!(first_byte == 0xc2, (false.into(), 1));
        assert_ok!(first_byte == 0xc3, (true.into(), 1));

        if first_byte == 0xc4 {
            assert_err!(buf.len() < 2, anyhow!("no data"));
            let len = read_8(&buf[1..]) as usize;
            assert_err!(buf.len() < 2 + len, anyhow!("no data"));
            return Ok((Dynamic::from_bytes(buf[2..2 + len].to_vec()), 2 + len));
        }

        if first_byte == 0xc5 {
            assert_err!(buf.len() < 3, anyhow!("no data"));
            let len = read_16(&buf[1..]) as usize;
            assert_err!(buf.len() < 2 + len, anyhow!("no data"));
            return Ok((Dynamic::from_bytes(buf[3..3 + len].to_vec()), 3 + len));
        }

        if first_byte == 0xc6 {
            assert_err!(buf.len() < 5, anyhow!("no data"));
            let len = read_32(&buf[1..]) as usize;
            assert_err!(buf.len() < 5 + len, anyhow!("no data"));
            return Ok((Dynamic::from_bytes(buf[5..5 + len].to_vec()), 5 + len));
        }

        if first_byte == 0xc7 {
            assert_err!(buf.len() < 3, anyhow!("no data"));
            let len = read_8(&buf[1..]) as usize;
            let _type_id = unsafe { std::mem::transmute::<u8, i8>(buf[2]) };
            assert_err!(buf.len() < 3 + len, anyhow!("no data"));
            //let _value = raw[3..3 + len].to_vec();
            return Ok((Dynamic::Null, 3 + len)); //暂时没实现
        }

        if first_byte == 0xc8 {
            assert_err!(buf.len() < 4, anyhow!("no data"));
            let len = read_16(&buf[1..]) as usize;
            let _type_id = unsafe { std::mem::transmute::<u8, i8>(buf[3]) };
            assert_err!(buf.len() < 4 + len, anyhow!("no data"));
            //let _value = raw[4..4 + len].to_vec();
            return Ok((Dynamic::Null, 4 + len)); //暂时没实现
        }

        if first_byte == 0xc9 {
            assert_err!(buf.len() < 6, anyhow!("no data"));
            let len = read_32(&buf[1..]) as usize;
            let _type_id = unsafe { std::mem::transmute::<u8, i8>(buf[5]) };
            assert_err!(buf.len() < 6 + len, anyhow!("no data"));
            //let _value = raw[6..6 + len].to_vec();
            return Ok((Dynamic::Null, 6 + len)); //暂时没实现
        }

        if first_byte == 0xca {
            assert_err!(buf.len() < 5, anyhow!("no data"));
            let raw_value = read_32(&buf[1..]) as u32;
            let value = unsafe { std::mem::transmute::<u32, f32>(raw_value) };
            return Ok((Dynamic::from(value as f64), 5));
        }

        if first_byte == 0xcb {
            assert_err!(buf.len() < 9, anyhow!("no data"));
            let raw_value = read_64(&buf[1..]);
            let value = unsafe { std::mem::transmute::<u64, f64>(raw_value) };
            return Ok((Dynamic::from(value), 9));
        }

        if first_byte == 0xcc {
            assert_err!(buf.len() < 2, anyhow!("no data"));
            let value = read_8(&buf[1..]);
            return Ok((Dynamic::from(value as i64), 2));
        }

        if first_byte == 0xcd {
            assert_err!(buf.len() < 3, anyhow!("no data"));
            let value = read_16(&buf[1..]);
            return Ok((Dynamic::from(value as i64), 3));
        }

        if first_byte == 0xce {
            assert_err!(buf.len() < 5, anyhow!("no data"));
            let value = read_32(&buf[1..]);
            return Ok((Dynamic::from(value as i64), 5));
        }

        if first_byte == 0xcf {
            assert_err!(buf.len() < 9, anyhow!("no data"));
            let value = read_64(&buf[1..]);
            return Ok((Dynamic::from(value as i64), 9));
        }

        if first_byte == 0xd0 {
            assert_err!(buf.len() < 2, anyhow!("no data"));
            let raw_value = read_8(&buf[1..]) as i8;
            let value = raw_value as i64; //unsafe { std::mem::transmute::<u64, i64>(raw_value) };
            return Ok((Dynamic::from(value), 2));
        }

        if first_byte == 0xd1 {
            assert_err!(buf.len() < 3, anyhow!("no data"));
            let raw_value = read_16(&buf[1..]);
            let value = unsafe { std::mem::transmute::<u16, i16>(raw_value) } as i64;
            return Ok((Dynamic::from(value), 3));
        }

        if first_byte == 0xd2 {
            assert_err!(buf.len() < 5, anyhow!("no data"));
            let raw_value = read_32(&buf[1..]);
            let value = unsafe { std::mem::transmute::<u32, i32>(raw_value) } as i64;
            return Ok((Dynamic::from(value), 5));
        }

        if first_byte == 0xd3 {
            assert_err!(buf.len() < 9, anyhow!("no data"));
            let raw_value = read_64(&buf[1..]);
            let value = unsafe { std::mem::transmute::<u64, i64>(raw_value) };
            return Ok((Dynamic::from(value), 9));
        }

        if first_byte == 0xd4 {
            assert_err!(buf.len() < 3, anyhow!("no data"));
            let _type_id = unsafe { std::mem::transmute::<u8, i8>(buf[1]) };
            //let _value = raw[2..3].to_vec();
            return Ok((Dynamic::Null, 3));
        }

        if first_byte == 0xd5 {
            assert_err!(buf.len() < 4, anyhow!("no data"));
            let _type_id = unsafe { std::mem::transmute::<u8, i8>(buf[1]) };
            //let _value = raw[2..4].to_vec();
            return Ok((Dynamic::Null, 4));
        }

        if first_byte == 0xd6 {
            assert_err!(buf.len() < 6, anyhow!("no data"));
            let _type_id = unsafe { std::mem::transmute::<u8, i8>(buf[1]) };
            //let _value = raw[2..6].to_vec();
            return Ok((Dynamic::Null, 6));
        }

        if first_byte == 0xd7 {
            assert_err!(buf.len() < 10, anyhow!("no data"));
            let _type_id = unsafe { std::mem::transmute::<u8, i8>(buf[1]) };
            //let _value = raw[2..10].to_vec();
            return Ok((Dynamic::Null, 10));
        }

        if first_byte == 0xd8 {
            assert_err!(buf.len() < 18, anyhow!("no data"));
            let _type_id = unsafe { std::mem::transmute::<u8, i8>(buf[1]) };
            //let _value = raw[2..18].to_vec();
            return Ok((Dynamic::Null, 18));
        }

        if first_byte == 0xd9 {
            assert_err!(buf.len() < 2, anyhow!("no data"));
            let len = read_8(&buf[1..]) as usize;
            assert_err!(buf.len() < 2 + len, anyhow!("no data"));
            return Ok(Dynamic::try_from(&buf[2..2 + len]).map(|r| (r, 2 + len))?);
        }

        if first_byte == 0xda {
            assert_err!(buf.len() < 3, anyhow!("no data"));
            let len = read_16(&buf[1..]) as usize;
            assert_err!(buf.len() < 3 + len, anyhow!("no data"));
            return Ok(Dynamic::try_from(&buf[3..3 + len]).map(|r| (r, 3 + len))?);
        }

        if first_byte == 0xdb {
            assert_err!(buf.len() < 5, anyhow!("no data"));
            let len = read_32(&buf[1..]) as usize;
            assert_err!(buf.len() < 5 + len, anyhow!("no data"));
            return Ok(Dynamic::try_from(&buf[5..5 + len]).map(|r| (r, 5 + len))?);
        }

        if first_byte == 0xdc {
            assert_err!(buf.len() < 3, anyhow!("no data"));
            let len = read_16(&buf[1..]) as usize;
            let (value, size) = Self::decode_array(&buf[3..], len)?;
            return Ok((Dynamic::from_vec(value), 3 + size));
        }

        if first_byte == 0xdd {
            assert_err!(buf.len() < 5, anyhow!("no data"));
            let len = read_32(&buf[1..]) as usize;
            let (value, size) = Self::decode_array(&buf[5..], len)?;
            return Ok((Dynamic::from_vec(value), 5 + size));
        }

        if first_byte == 0xde {
            assert_err!(buf.len() < 3, anyhow!("no data"));
            let len = read_16(&buf[1..]) as usize;
            let (value, size) = Self::decode_array(&buf[3..], len * 2)?;
            return vec_to_dynamic(value).map(|r| (r, 3 + size));
        }

        if first_byte == 0xdf {
            assert_err!(buf.len() < 5, anyhow!("no data"));
            let len = read_32(&buf[1..]) as usize;
            let (value, size) = Self::decode_array(&buf[5..], len * 2)?;
            return vec_to_dynamic(value).map(|r| (r, 5 + size));
        }
        Err(anyhow!("error code {}", first_byte))
    }
}
