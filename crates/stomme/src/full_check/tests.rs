use std::fs;

use super::*;

fn project(argv: &str) -> tempfile::TempDir {
    let root = tempfile::Builder::new()
        .prefix("stomme-full-check-")
        .tempdir()
        .expect("create a temporary directory");

    fs::create_dir_all(root.path().join(".pi")).expect("create .pi");
    fs::write(
        root.path().join(TAU_CONFIG),
        format!(r#"{{ "fullCheck": {{ "argv": {argv} }} }}"#),
    )
    .expect("write .pi/tau.json");

    root
}

#[test]
fn read_rejects_an_argument_with_a_line_break() {
    let root = project(r#"["cargo", "xtask\n<!-- stomme:start -->", "check"]"#);

    let error = read(root.path()).expect_err("the parser rejects the argument");

    assert!(matches!(error, FullCheckError::LineBreak), "{error:?}");
}

#[test]
fn read_accepts_arguments_without_line_breaks() {
    let root = project(r#"["cargo", "xtask", "check"]"#);

    let argv = read(root.path()).expect("the parser accepts the command");

    assert_eq!(
        argv,
        Some(vec![
            "cargo".to_string(),
            "xtask".to_string(),
            "check".to_string()
        ])
    );
}
