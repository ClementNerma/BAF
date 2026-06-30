use std::io::Cursor;

use bolero::TypeGenerator;

#[derive(Debug, TypeGenerator)]
enum Op {
    /// Create a file in the root directory
    FileRoot { name: String, content: Vec<u8> },
    /// Create a directory in the root
    DirRoot { name: String },
    /// Create a file in a named subdirectory (will be created if needed)
    FileInDir {
        dir: String,
        name: String,
        content: Vec<u8>,
    },
}

#[test]
fn fuzz_roundtrip() {
    bolero::check!().with_type::<Vec<Op>>().for_each(|ops| {
        let cursor = Cursor::new(Vec::new());
        let mut expected: Vec<(String, Vec<u8>)> = Vec::new();
        let mut expected_dirs: Vec<String> = Vec::new();

        let mut archive = match crate::Archive::create(cursor, crate::ArchiveConfig::default()) {
            Ok(a) => a,
            Err(_) => return,
        };

        for op in ops {
            match op {
                Op::FileRoot { name, content } => {
                    let name = match crate::ItemName::new(name.clone()) {
                        Ok(n) => n,
                        Err(_) => continue,
                    };
                    let _ = archive.create_file(
                        crate::DirectoryIdOrRoot::Root,
                        name.clone(),
                        crate::Timestamp::now(),
                        Cursor::new(content.clone()),
                    );
                    expected.push((name.into_string(), content.clone()));
                }
                Op::DirRoot { name } => {
                    let name = match crate::ItemName::new(name.clone()) {
                        Ok(n) => n,
                        Err(_) => continue,
                    };
                    let _ = archive.create_dir(
                        crate::DirectoryIdOrRoot::Root,
                        name.clone(),
                        crate::Timestamp::now(),
                    );
                    expected_dirs.push(name.into_string());
                }
                Op::FileInDir { dir, name, content } => {
                    let dir_name = match crate::ItemName::new(dir.clone()) {
                        Ok(n) => n,
                        Err(_) => continue,
                    };
                    let file_name = match crate::ItemName::new(name.clone()) {
                        Ok(n) => n,
                        Err(_) => continue,
                    };
                    let dir_id = archive
                        .create_dir(
                            crate::DirectoryIdOrRoot::Root,
                            dir_name.clone(),
                            crate::Timestamp::now(),
                        )
                        .unwrap_or_else(|_| {
                            archive
                                .create_dir(
                                    crate::DirectoryIdOrRoot::Root,
                                    dir_name.clone(),
                                    crate::Timestamp::now(),
                                )
                                .expect("fallback create_dir failed")
                        });
                    let _ = archive.create_file(
                        crate::DirectoryIdOrRoot::NonRoot(dir_id),
                        file_name.clone(),
                        crate::Timestamp::now(),
                        Cursor::new(content.clone()),
                    );
                    expected.push((
                        format!("{}/{}", dir_name.into_string(), file_name.into_string()),
                        content.clone(),
                    ));
                }
            }
        }

        let cursor = match archive.close() {
            Ok(c) => c,
            Err(_) => return,
        };

        let data = cursor.into_inner();
        if data.is_empty() {
            return;
        }

        let mut archive =
            match crate::Archive::open(Cursor::new(data), crate::ArchiveConfig::default()) {
                Ok(a) => a,
                Err(_) => return,
            };

        for (expected_path, expected_content) in &expected {
            if let Some(file) = archive.with_paths().get_file_at(expected_path) {
                let content = archive.read_file_to_vec(file.id);
                if let Ok(content) = content {
                    assert_eq!(
                        &content, expected_content,
                        "file '{expected_path}' content mismatch"
                    );
                }
            }
        }

        for expected_dir in &expected_dirs {
            let _ = archive.with_paths().get_dir_at(expected_dir);
        }
    });
}
