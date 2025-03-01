use anyhow::Result;
use libai::dynamic::Dynamic;
use libai::dynamic;
use libai::json::ToJson;
use libai::msgpack::{MsgPack, MsgUnpack};
use rune::{ToValue, Value};

fn main() -> Result<()> {
    let obj = dynamic!("name"=> "zhu", "age"=>10, "other"=>vec![Dynamic::from(10), Dynamic::from("aa"), Dynamic::from(30.0)]);
    let mut buf = String::new();
    obj.to_json(&mut buf);
    println!("{:?}", obj);
    println!("{}", buf);
    
    let mut buf = Vec::new();
    obj.encode(&mut buf);
    let obj = Dynamic::decode(buf.as_slice());
    println!("{} {:?}", buf.len(), obj);
    let value = rune::Value::from(&obj.unwrap().0);
    println!("{:?}", value);
    let d = Dynamic::from(&value);
    println!("{:?}", d);
    let b1 = Dynamic::from_bytes(vec![1,2,3,4,5]);
    let b2 = Dynamic::from_bytes(vec![1,2,3,4,5]);
    let b3 = Dynamic::from_bytes(vec![1,2,3,4,5,6]);
    let b4 = Dynamic::from_bytes(vec![1,2,0,4,5]);
    println!("{} {} {} {}", b1 == b2, b1 == b3, b1 == b4, b2 == b1);
    let ddd = Dynamic::from_vec(vec![Dynamic::from(1), Dynamic::from(2), Dynamic::from("aaaaaa")]);
    let v = (Value::from(1i64), rune::alloc::String::try_from("aaaaa")?.to_value().unwrap()).to_value().unwrap();
    let dvec = ddd.into_array().unwrap();
    println!("{:?}", Dynamic::from(&v));
    Ok(())
}