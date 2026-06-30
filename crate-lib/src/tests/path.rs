use crate::{PathError, PathInArchive};

#[test]
fn test_simple() {
    let path = PathInArchive::new("a/b/c").unwrap();
    let components: Vec<_> = path.components().iter().map(|c| c.as_ref()).collect();
    assert_eq!(components, vec!["a", "b", "c"]);
}

#[test]
fn test_empty() {
    let err = PathInArchive::new("").unwrap_err();
    assert!(matches!(err, PathError::EmptyPath));
}

#[test]
fn test_current_dir_dot() {
    let path = PathInArchive::new("a/./b").unwrap();
    let components: Vec<_> = path.components().iter().map(|c| c.as_ref()).collect();
    assert_eq!(components, vec!["a", "b"]);
}

#[test]
fn test_parent_dir_dotdot() {
    let path = PathInArchive::new("a/b/../c").unwrap();
    let components: Vec<_> = path.components().iter().map(|c| c.as_ref()).collect();
    assert_eq!(components, vec!["a", "c"]);
}

#[test]
fn test_leading_slash_stripped() {
    let path = PathInArchive::new("/a/b").unwrap();
    let components: Vec<_> = path.components().iter().map(|c| c.as_ref()).collect();
    assert_eq!(components, vec!["a", "b"]);
}

#[test]
fn test_double_slash() {
    let path = PathInArchive::new("a//b").unwrap();
    let components: Vec<_> = path.components().iter().map(|c| c.as_ref()).collect();
    assert_eq!(components, vec!["a", "b"]);
}

#[test]
fn test_trailing_slash() {
    let path = PathInArchive::new("a/b/").unwrap();
    let components: Vec<_> = path.components().iter().map(|c| c.as_ref()).collect();
    assert_eq!(components, vec!["a", "b"]);
}

#[test]
fn test_dotdot_beyond_root() {
    assert!(PathInArchive::new("../..").is_err());
}

#[test]
fn test_from_components() {
    let path = PathInArchive::from_components(&["x", "y", "z"]).unwrap();
    let components: Vec<_> = path.components().iter().map(|c| c.as_ref()).collect();
    assert_eq!(components, vec!["x", "y", "z"]);
}

#[test]
fn test_invalid_component() {
    let err = PathInArchive::new("valid/\n_bad_comp").unwrap_err();
    assert!(matches!(err, PathError::InvalidComponent { .. }));
}

#[test]
fn test_empty_constructor() {
    let path = PathInArchive::empty();
    assert!(path.is_empty());
    assert!(path.components().is_empty());
    assert!(path.filename().is_none());
    assert!(path.parent().is_none());
}

#[test]
fn test_parent_none() {
    let path = PathInArchive::new("a").unwrap();
    assert!(path.parent().is_none());
}

#[test]
fn test_parent_some() {
    let path = PathInArchive::new("a/b").unwrap();
    let parent = path.parent().unwrap();
    let components: Vec<_> = parent.components().iter().map(|c| c.as_ref()).collect();
    assert_eq!(components, vec!["a"]);
}

#[test]
fn test_filename() {
    let path = PathInArchive::new("a/b/c").unwrap();
    assert_eq!(path.filename().unwrap().as_ref(), "c");
}

#[test]
fn test_filename_single() {
    let path = PathInArchive::new("a").unwrap();
    assert_eq!(path.filename().unwrap().as_ref(), "a");
}

#[test]
fn test_pop() {
    let mut path = PathInArchive::new("a/b/c").unwrap();
    assert_eq!(path.pop().unwrap().as_ref(), "c");
    assert_eq!(path.pop().unwrap().as_ref(), "b");
    assert_eq!(path.pop().unwrap().as_ref(), "a");
    assert!(path.pop().is_none());
    assert!(path.is_empty());
}

#[test]
fn test_append_and_join() {
    let mut path = PathInArchive::empty();
    let comp = crate::ItemName::new("first".to_owned()).unwrap();
    path.append(comp);
    let comp = crate::ItemName::new("second".to_owned()).unwrap();
    let path = path.join(comp);
    let components: Vec<_> = path.components().iter().map(|c| c.as_ref()).collect();
    assert_eq!(components, vec!["first", "second"]);
}

#[test]
fn test_append_str_and_join_str() {
    let mut path = PathInArchive::empty();
    path.append_str("first").unwrap();
    let path = path.join_str("second").unwrap();
    let components: Vec<_> = path.components().iter().map(|c| c.as_ref()).collect();
    assert_eq!(components, vec!["first", "second"]);
}

#[test]
fn test_display() {
    let path = PathInArchive::new("a/b/c").unwrap();
    assert_eq!(format!("{path}"), "a/b/c");
}

#[test]
fn test_display_empty() {
    let path = PathInArchive::empty();
    assert_eq!(format!("{path}"), "");
}

#[test]
fn test_append_str_forbidden() {
    let mut path = PathInArchive::empty();
    let err = path.append_str("cont/ains_slash").unwrap_err();
    assert!(matches!(
        err,
        crate::NameValidationError::ForbiddenChar('/')
    ));
}

#[test]
fn test_path_error_display() {
    assert_eq!(format!("{}", PathError::EmptyPath), "Path cannot be empty");
    let invalid = PathError::InvalidComponent {
        path: "/bad/name".to_owned(),
        component: "bad/name".to_owned(),
        err: crate::NameValidationError::ForbiddenChar('/'),
    };
    let displayed = format!("{invalid}");
    assert!(displayed.contains("/bad/name"));
    assert!(displayed.contains("bad/name"));
    assert!(displayed.contains("invalid"));
}
