use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use tempfile::{Builder, TempDir};

pub(crate) fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

pub(crate) fn temporary_directory() -> TempDir {
    Builder::new()
        .prefix("stomme-xtask-test-")
        .tempdir()
        .expect("create a temporary directory")
}

pub(crate) fn write_executable(path: &Path, text: &str) {
    fs::write(path, text).expect("write the fake command");
    let mut permissions = fs::metadata(path)
        .expect("the fake command exists")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("make the fake command executable");
}

// A throwaway line that the scoped OCR review must cover, amended.
