#[test]
fn fuzz_archive_open() {
    bolero::check!().for_each(|data: &[u8]| {
        let cursor = std::io::Cursor::new(data.to_vec());
        let _ = crate::Archive::open(cursor, crate::ArchiveConfig::default());
    });
}
