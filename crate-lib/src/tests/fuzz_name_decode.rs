#[test]
fn fuzz_name_decode() {
    bolero::check!().for_each(|data: &[u8]| {
        if data.len() == 256 {
            let bytes: [u8; 256] = data.try_into().unwrap();
            let _ = crate::ItemName::decode(bytes);
        }
    });
}
