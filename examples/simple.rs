use anyhow::Result;
use libai::dynamic::Dynamic;
use libai::dynamic;
use libai::json::ToJson;
use libai::msgpack::{MsgPack, MsgUnpack};
use rune::Value;

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
    Ok(())
}