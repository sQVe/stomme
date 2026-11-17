use tempfile::TempDir;

use crate::test_support::write_file;

pub(crate) fn project(files: &[(&str, &str)], table: &str) -> TempDir {
    let root = tempfile::tempdir().expect("create a temporary directory");
    let config = format!(
        "schema = 1\nversion = \"{}\"\nlanguage = \"rust\"\n\n{table}",
        env!("CARGO_PKG_VERSION")
    );

    write_file(root.path(), "stomme.toml", &config);

    for (path, text) in files {
        write_file(root.path(), path, text);
    }

    root
}

pub(crate) fn lock_text(packages: &[(&str, &str, &[&str])]) -> String {
    let mut text = "version = 4\n".to_string();

    for (name, version, dependencies) in packages {
        text.push_str(&format!(
            "\n[[package]]\nname = \"{name}\"\nversion = \"{version}\"\n"
        ));

        if !dependencies.is_empty() {
            let entries: Vec<String> = dependencies
                .iter()
                .map(|dependency| format!("\"{dependency}\""))
                .collect();

            text.push_str(&format!("dependencies = [{}]\n", entries.join(", ")));
        }
    }

    text
}
