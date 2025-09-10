use commy_macro::rkyv_writer;

#[rkyv_writer]
#[derive(::rkyv::Archive, ::rkyv::Serialize, ::rkyv::Deserialize, Debug, PartialEq)]
pub struct TestStruct {
    a: u32,
    b: String,
}

#[test]
fn compiles_and_writes() {
    let t = TestStruct {
        a: 42,
        b: "hello".to_string(),
    };
    let mut buf = vec![0u8; 1024];
    let n = t.write_into_buffer(&mut buf).expect("write");
    assert!(n > 0);
}
