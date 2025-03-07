use anyhow::Result;
use libai::dynamic::Dynamic;
use libai::{dmap, dvec};
use libai::json::ToJson;
use libai::msgpack::{MsgPack, MsgUnpack};

fn main() -> Result<()> {
    let obj = dmap!("name"=> "zhu", "age"=>10, "other"=>dvec![10, "aa", 30.0]);
    let mut buf = String::new();
    obj.to_json(&mut buf);
    println!("{:?}", obj);
    println!("{}", buf);
    
    let mut buf = Vec::new();
    obj.encode(&mut buf);
    let obj = Dynamic::decode(buf.as_slice());
    println!("{} {:?}", buf.len(), obj);
    let b1 = Dynamic::from_bytes(vec![1,2,3,4,5]);
    let b2 = Dynamic::from_bytes(vec![1,2,3,4,5]);
    let b3 = Dynamic::from_bytes(vec![1,2,3,4,5,6]);
    let b4 = Dynamic::from_bytes(vec![1,2,0,4,5]);
    println!("{} {} {} {}", b1 == b2, b1 == b3, b1 == b4, b2 == b1);
    Ok(())
}