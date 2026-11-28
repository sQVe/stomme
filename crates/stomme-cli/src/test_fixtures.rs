use std::fs;
use std::path::Path;

// Shared fixtures name version 0.0.0 so that a release bump changes no fixture.
pub(crate) fn with_crate_version(config: &str) -> String {
    let version = env!("CARGO_PKG_VERSION");

    config.replacen(
        "version = \"0.0.0\"",
        &format!("version = \"{version}\""),
        1,
    )
}

pub(crate) fn copy_tree(source: &Path, target: &Path) {
    for entry in fs::read_dir(source).expect("read the fixture folder") {
        let entry = entry.expect("read a fixture entry");
        let path = entry.path();
        let destination = target.join(entry.file_name());

        if entry.file_type().expect("read the file type").is_dir() {
            fs::create_dir_all(&destination).expect("create the fixture folder");
            copy_tree(&path, &destination);
        } else {
            fs::copy(&path, &destination).expect("copy the fixture file");
        }
    }
}
