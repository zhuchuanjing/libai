use anyhow::Result;
use libai::dynamic::Dynamic;
use libai::dynamic;
use libai::json::ToJson;
use libai::msgpack::{MsgPack, MsgUnpack};

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
    Ok(())
}