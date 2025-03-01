use std::{ops::Deref, str::Utf8Error, sync::{Arc, RwLock}};
use smol_str::SmolStr;
use std::collections::BTreeMap;
use anyhow::{Result, anyhow};

use rune::Any;
//自己定义一个 Dynamic 类型 支持 不同脚本语言的类型的自由转化
#[derive(Debug, Clone, Any)]
pub enum Dynamic {
    Null,
    Bool(bool),
    Byte(u8),
    Int(i64),
    UInt(u64),
    Float(f32),
    Double(f64),
    String(Arc<SmolStr>),                           //----上面这些值可以直接修改
    Array(Arc<RwLock<Vec<Dynamic>>>),
    Object(Arc<RwLock<BTreeMap<SmolStr, Dynamic>>>),
    Bytes(Arc<Vec<u8>>),
}

unsafe impl Send for Dynamic {}
unsafe impl Sync for Dynamic {}

impl Default for Dynamic {
    fn default() -> Self {
        Self::Null
    } 
}

impl Dynamic {
    pub fn object()-> Self {
        Self::Object(Arc::new(RwLock::new(BTreeMap::new())))
    }

    pub fn from_map(map: BTreeMap<SmolStr, Dynamic>)-> Self {
        Self::Object(Arc::new(RwLock::new(map)))
    }

    pub fn array()-> Self {
        Self::Array(Arc::new(RwLock::new(Vec::new())))
    }

    pub fn from_vec(vec: Vec<Dynamic>)-> Self {
        Self::Array(Arc::new(RwLock::new(vec)))
    }
    
    pub fn from_bytes(vec: Vec<u8>)-> Self {
        Self::Bytes(Arc::new(vec))
    }

    pub fn is_null(&self)-> bool {
        match self {
            Self::Null=> true,
            _=> false
        }
    }
    
    pub fn is_string(&self)-> bool {
        match self {
            Self::String(_)=> true,
            _=> false
        }
    }
    
    pub fn as_str(&self)-> Result<&str> {
        match self {
            Self::String(s)=> Ok(s.as_str()),
            _=> Err(anyhow!("is not a String"))
        }
    }

    pub fn as_u64(&self)-> Result<u64> {
        match self {
            Self::Int(i)=> Ok(*i as u64),
            Self::UInt(u)=> Ok(*u),
            _=> Err(anyhow!("is not a integer"))
        }
    }

    pub fn into_string(self)-> Result<SmolStr> {
        match self {
            Self::String(s)=> Ok(s.deref().clone()),
            _=> Err(anyhow!("not a string"))
        }
    }
    
    pub fn into_array(self)-> Result<Vec<Dynamic>> {
        match self {
            Self::Array(array)=> Ok(array.read().unwrap().clone()),
            _=> Err(anyhow!("not a array"))
        }
    }

    pub fn is_bool(&self)-> bool {
        match self {
            Self::Bool(_)=> true,
            _=> false
        }
    }
    pub fn is_array(&self)-> bool {
        match self {
            Self::Array(_)=> true,
            _=> false
        }
    }
    pub fn is_object(&self)-> bool {
        match self {
            Self::Object(_)=> true,
            _=> false
        }
    }

    pub fn as_bool(&self)-> Result<bool> {
        match self {
            Self::Bool(b)=> Ok(*b),
            _=> Err(anyhow!("is not a bool"))
        }
    }
    
    pub fn len(&self)-> Result<usize> {
        match self {
            Self::Array(array)=> {
                Ok(array.read().unwrap().len())
            },
            Self::Object(obj)=> {
                Ok(obj.read().unwrap().len())
            },
            _=> Err(anyhow!("is not a array"))
        }
    }
    
    pub fn get(&self, index: usize)-> Result<Dynamic> {
        match self {
            Self::Array(array)=> {
                array.read().unwrap().get(index).map(|item| item.clone() ).ok_or(anyhow!("index {} is outbound", index))
            },
            _=> Err(anyhow!("is not a array"))
        }
    }

    pub fn push<T: Into<Dynamic>>(&self, val: T)-> Result<()> {
        match self {
            Self::Array(array)=> {
                array.write().unwrap().push(val.into());
                Ok(())
            },
            _=> Err(anyhow!("is not a array"))
        }
    }

    pub fn pop(&self)-> Result<Dynamic> {
        match self {
            Self::Array(array)=> {
                array.write().unwrap().pop().ok_or(anyhow!("no more items"))
            },
            _=> Err(anyhow!("is not a array"))
        }
    }

    pub fn get_key(&self, key: &str)-> Result<Dynamic> {
        match self {
            Self::Object(obj)=> {
                obj.read().unwrap().get(key).map(|item| item.clone() ).ok_or(anyhow!("key {} is not existed", key))
            },
            _=> Err(anyhow!("is not a object"))
        }      
    }
    
    pub fn set_key<T: Into<Dynamic>>(&self, key: &str, val: T)-> Result<Option<Dynamic>> {
        match self {
            Self::Object(obj)=> {
                Ok(obj.write().unwrap().insert(SmolStr::new(key), val.into()))
            },
            _=> Err(anyhow!("is not a object"))
        }      
    }

    pub fn remove_key(&self, key: &str)-> Result<Option<Dynamic>> {
        match self {
            Self::Object(obj)=> {
                Ok(obj.write().unwrap().remove(key))
            },
            _=> Err(anyhow!("is not a object"))
        }      
    }     

    pub fn contains(&self, key: &str)-> Result<bool> {
        match self {
            Self::Object(obj)=> {
                Ok(obj.read().unwrap().contains_key(key))
            },
            _=> Err(anyhow!("is not a object"))
        }      
    }  
    
    pub fn append(&self, other: &Dynamic)-> Result<()> {
        match self {
            Self::Object(obj)=> {
                match other {
                    Self::Object(other)=> {
                        for kv in other.read().unwrap().iter() {
                            obj.write().unwrap().insert(kv.0.clone(), kv.1.clone());
                        }
                        Ok(())
                    },
                    _=> Err(anyhow!("other is not a object"))
                }
            },
            Self::Array(array)=> {
                match other {
                    Self::Array(other)=> {
                        for item in other.read().unwrap().iter() {
                            array.write().unwrap().push(item.clone());
                        }
                        Ok(())
                    }
                    _=> Err(anyhow!("other is not a array"))
                }
            }
            _=> Err(anyhow!("is not a object"))
        }      
    }  
}

impl From<String> for Dynamic {
    fn from(s: String)-> Self {
        Self::String(Arc::new(SmolStr::from(s)))
    }
}

impl From<&str> for Dynamic {
    fn from(s: &str)-> Self {
        Self::String(Arc::new(SmolStr::from(s)))
    }
}

impl TryFrom<&[u8]> for Dynamic {
    type Error = Utf8Error;
    fn try_from(value: &[u8]) -> std::result::Result<Self, Self::Error> {
        let s = std::str::from_utf8(value).unwrap();
        Ok(Dynamic::from(s))
    }
}

impl From<bool> for Dynamic {
    fn from(b: bool)-> Self {
        if b { Self::Bool(true) } else { Self::Bool(false) }
    }
}

impl From<f64> for Dynamic {
    fn from(f: f64)-> Self {
        Self::Double(f)
    }
}

impl From<i64> for Dynamic {
    fn from(i: i64)-> Self {
        Self::Int(i)
    }
}

impl From<Vec<Dynamic>> for Dynamic {
    fn from(v: Vec<Dynamic>)-> Self {
        Self::from_vec(v)
    }
}

impl PartialEq for Dynamic {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Self::Null=> {
                if let Self::Null = other { true }
                else { false }
            }
            Self::Bool(b)=> {
                if let Self::Bool(o) = other { *b == * o }
                else { false }
            }
            Self::Byte(b)=> {
                if let Self::Byte(o) = other { *b == * o }
                else { false }
            }
            Self::Int(b)=> {
                if let Self::Int(o) = other { *b == * o }
                else { false }
            }
            Self::UInt(b)=> {
                if let Self::UInt(o) = other { *b == * o }
                else { false }
            }
            Self::Float(b)=> {
                if let Self::Float(o) = other { *b == * o }
                else { false }
            }
            Self::Double(b)=> {
                if let Self::Double(o) = other { *b == * o }
                else { false }
            }
            Self::String(s1)=> {
                if let Self::String(s2) = other { s1.eq(s2) }
                else { false }
            }
            Self::Array(_)=> false,
            Self::Object(_)=> false,
            Self::Bytes(b1)=> {
                if let Self::Bytes(b2) = other { b1.as_slice() == b2.as_slice() }
                else { false }
            }
        }
    }
}