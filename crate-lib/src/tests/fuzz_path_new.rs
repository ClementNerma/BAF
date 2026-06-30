#[test]
fn fuzz_path_new() {
    bolero::check!().for_each(|data: &[u8]| {
        if let Ok(s) = std::str::from_utf8(data) {
            let _ = crate::PathInArchive::new(s);
        }
    });
}
