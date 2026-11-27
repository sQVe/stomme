use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

use super::*;
use crate::test_support::{commit, git};

const BASE_KEPT: &str = "\
kept first
kept second
kept third
kept fourth
kept fifth
";

const REVIEWED_KEPT: &str = "\
kept first
kept second reviewed
kept third
kept fourth
kept fifth
";

const BASE_CHANGED: &str = "\
changed first
changed second
changed third
changed fourth
changed fifth
";

const REVIEWED_CHANGED: &str = "\
changed first
changed second reviewed
changed third
changed fourth
changed fifth
";

const HEAD_CHANGED: &str = "\
changed first
changed second reviewed
changed third
changed fourth added
changed fourth
changed fifth
";

const BASE_MAIN_ONLY: &str = "\
main first
main second
main third
main fourth
main fifth
";

const MOVED_MAIN_ONLY: &str = "\
main first
main second
main third
main fourth moved
main fifth
";

const BASE_FILES: &[(&str, &str)] = &[
    ("src/kept.rs", BASE_KEPT),
    ("src/changed.rs", BASE_CHANGED),
    ("src/main_only.rs", BASE_MAIN_ONLY),
];

const REVIEWED_FILES: &[(&str, &str)] = &[
    ("src/kept.rs", REVIEWED_KEPT),
    ("src/changed.rs", REVIEWED_CHANGED),
];

const MOVED_BASE_FILES: &[(&str, &str)] = &[
    ("src/kept.rs", BASE_KEPT),
    ("src/changed.rs", BASE_CHANGED),
    ("src/main_only.rs", MOVED_MAIN_ONLY),
];

const HEAD_FILES: &[(&str, &str)] = &[
    ("src/kept.rs", REVIEWED_KEPT),
    ("src/changed.rs", HEAD_CHANGED),
    ("src/main_only.rs", MOVED_MAIN_ONLY),
];

struct History {
    base: String,
    reviewed: String,
    moved_base: String,
    head: String,
}

#[test]
fn scopes_only_the_amended_line_after_a_rebase() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| write_files(folder, BASE_FILES),
        |folder| write_files(folder, REVIEWED_FILES),
        |folder| write_files(folder, MOVED_BASE_FILES),
        |folder| write_files(folder, HEAD_FILES),
    );

    let outcome = run_scope(folder.path(), &history).expect("build the scope base");
    let Outcome::Base {
        commit: base,
        files,
        added,
        removed,
    } = outcome
    else {
        panic!("expected a base commit, got {outcome:?}");
    };

    assert_eq!((files, added, removed), (1, 1, 0));
    assert_eq!(
        added_texts(folder.path(), &base, &history.head),
        ["changed fourth added"]
    );
    assert!(removed_texts(folder.path(), &base, &history.head).is_empty());
    assert_eq!(
        changed_paths(folder.path(), &base, &history.head),
        ["src/changed.rs"]
    );
}

#[test]
fn scopes_every_added_line_on_an_amend_force_push() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| write_files(folder, &[("src/file.rs", "one\ntwo\nthree\n")]),
        |folder| write_files(folder, &[("src/file.rs", "one\ntwo reviewed\nthree\n")]),
        |folder| write_files(folder, &[("src/file.rs", "one\ntwo\nthree\n")]),
        |folder| write_files(folder, &[("src/file.rs", "one\ntwo amended\nthree\n")]),
    );

    let outcome = run_scope(folder.path(), &history).expect("build the scope base");
    let Outcome::Base {
        commit: base,
        files,
        added,
        removed,
    } = outcome
    else {
        panic!("expected a base commit, got {outcome:?}");
    };

    assert_eq!((files, added, removed), (1, 1, 0));
    assert_eq!(
        added_texts(folder.path(), &base, &history.head),
        ["two amended"]
    );
    assert!(removed_texts(folder.path(), &base, &history.head).is_empty());
}

#[test]
fn covers_a_moved_block() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| write_files(folder, &[("src/file.rs", "one\ntwo\nthree\nfour\n")]),
        |folder| {
            write_files(
                folder,
                &[(
                    "src/file.rs",
                    "block one\nblock two\nblock three\none\ntwo\nthree\nfour\n",
                )],
            )
        },
        |folder| write_files(folder, &[("src/file.rs", "one\ntwo\nthree\nfour\n")]),
        |folder| {
            write_files(
                folder,
                &[(
                    "src/file.rs",
                    "one\ntwo\nthree\nfour\nblock one\nblock two\nblock three\n",
                )],
            )
        },
    );

    let outcome = run_scope(folder.path(), &history).expect("build the scope base");

    assert_eq!(outcome, Outcome::Empty);
    assert!(replace_refs(folder.path()).is_empty());
}

#[test]
fn scopes_an_inserted_duplicate_of_a_covered_line() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| write_files(folder, &[("src/file.rs", "alpha\nbeta\n")]),
        |folder| write_files(folder, &[("src/file.rs", "alpha\nbeta\ncovered\n")]),
        |folder| write_files(folder, &[("src/file.rs", "alpha\nbeta\n")]),
        |folder| {
            write_files(
                folder,
                &[("src/file.rs", "alpha\nbeta\ncovered\ncovered\n")],
            )
        },
    );

    let outcome = run_scope(folder.path(), &history).expect("build the scope base");
    let Outcome::Base {
        commit: base,
        files,
        added,
        removed,
    } = outcome
    else {
        panic!("expected a base commit, got {outcome:?}");
    };

    assert_eq!((files, added, removed), (1, 1, 0));
    assert_eq!(
        added_texts(folder.path(), &base, &history.head),
        ["covered"]
    );
    assert_eq!(
        numstat_text(folder.path(), &base, &history.head),
        "1\t0\tsrc/file.rs"
    );
}

#[test]
fn follows_a_renamed_file() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| write_files(folder, &[("src/old.rs", "one\ntwo\nthree\nfour\nfive\n")]),
        |folder| {
            write_files(
                folder,
                &[("src/old.rs", "one\ntwo reviewed\nthree\nfour\nfive\n")],
            )
        },
        |folder| write_files(folder, &[("src/old.rs", "one\ntwo\nthree\nfour\nfive\n")]),
        |folder| {
            remove_files(folder, &["src/old.rs"]);
            write_files(
                folder,
                &[("src/new.rs", "one\ntwo reviewed\nthree\nfour\nfive\n")],
            );
        },
    );

    let outcome = run_scope(folder.path(), &history).expect("build the scope base");

    assert_eq!(outcome, Outcome::Empty);
    assert!(replace_refs(folder.path()).is_empty());
}

#[test]
fn scopes_a_line_removed_since_the_review() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| {
            write_files(
                folder,
                &[("src/file.rs", "keep first\nremove me\nkeep last\n")],
            )
        },
        |folder| {
            write_files(
                folder,
                &[(
                    "src/file.rs",
                    "keep first\nremove me\nkeep last\nreviewed addition\n",
                )],
            )
        },
        |folder| {
            write_files(
                folder,
                &[("src/file.rs", "keep first\nremove me\nkeep last\n")],
            )
        },
        |folder| {
            write_files(
                folder,
                &[("src/file.rs", "keep first\nkeep last\nreviewed addition\n")],
            )
        },
    );

    let outcome = run_scope(folder.path(), &history).expect("build the scope base");
    let Outcome::Base {
        commit: base,
        files,
        added,
        removed,
    } = outcome
    else {
        panic!("expected a base commit, got {outcome:?}");
    };

    assert_eq!((files, added, removed), (1, 0, 1));
    assert!(added_texts(folder.path(), &base, &history.head).is_empty());
    assert_eq!(
        removed_texts(folder.path(), &base, &history.head),
        ["remove me"]
    );
}

#[test]
fn leaves_out_a_file_deleted_since_the_review() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| {
            write_files(
                folder,
                &[
                    ("src/a.txt", "alpha\nbeta\n"),
                    ("src/b.txt", "b first\nb second\n"),
                ],
            )
        },
        |folder| {
            write_files(
                folder,
                &[
                    ("src/a.txt", "alpha\nbeta\n"),
                    ("src/b.txt", "b first\nb second reviewed\n"),
                ],
            )
        },
        |folder| {
            write_files(
                folder,
                &[
                    ("src/a.txt", "alpha\nbeta\n"),
                    ("src/b.txt", "b first\nb second\n"),
                ],
            )
        },
        |folder| {
            remove_files(folder, &["src/a.txt"]);
            write_files(
                folder,
                &[("src/b.txt", "b first\nb second reviewed\nb new line\n")],
            );
        },
    );

    let outcome = run_scope(folder.path(), &history).expect("build the scope base");
    let Outcome::Base {
        commit: base,
        files,
        added,
        removed,
    } = outcome
    else {
        panic!("expected a base commit, got {outcome:?}");
    };

    assert_eq!((files, added, removed), (1, 1, 0));
    assert_eq!(
        added_texts(folder.path(), &base, &history.head),
        ["b new line"]
    );
    assert_eq!(
        changed_paths(folder.path(), &base, &history.head),
        ["src/b.txt"]
    );
}

#[test]
fn returns_empty_when_a_rebase_brings_no_new_lines() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| write_files(folder, &[("src/file.rs", "one\ntwo\nthree\n")]),
        |folder| {
            write_files(
                folder,
                &[("src/file.rs", "one\ntwo\nreviewed line\nthree\n")],
            )
        },
        |folder| write_files(folder, &[("src/file.rs", "zero\none\ntwo\nthree\n")]),
        |folder| {
            write_files(
                folder,
                &[("src/file.rs", "zero\none\ntwo\nreviewed line\nthree\n")],
            )
        },
    );

    let outcome = run_scope(folder.path(), &history).expect("build the scope base");

    assert_eq!(outcome, Outcome::Empty);
    assert!(replace_refs(folder.path()).is_empty());
}

#[test]
fn counts_a_whitespace_only_edit_as_new() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| write_files(folder, &[("src/file.rs", "alpha\n")]),
        |folder| write_files(folder, &[("src/file.rs", "alpha\nbeta\n")]),
        |folder| write_files(folder, &[("src/file.rs", "alpha\n")]),
        |folder| write_files(folder, &[("src/file.rs", "alpha\n beta\n")]),
    );

    let outcome = run_scope(folder.path(), &history).expect("build the scope base");
    let Outcome::Base {
        commit: base,
        files,
        added,
        removed,
    } = outcome
    else {
        panic!("expected a base commit, got {outcome:?}");
    };

    assert_eq!((files, added, removed), (1, 1, 0));
    assert_eq!(added_texts(folder.path(), &base, &history.head), [" beta"]);
}

#[test]
fn keeps_the_head_entry_of_a_binary_file() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| {
            write_files(folder, &[("src/file.rs", "one\n")]);
            write_file(folder, "assets/logo.bin", &[0, 1, 2, 3]);
        },
        |folder| {
            write_files(folder, &[("src/file.rs", "one\ntwo reviewed\n")]);
            write_file(folder, "assets/logo.bin", &[0, 1, 2, 3]);
        },
        |folder| {
            write_files(folder, &[("src/file.rs", "one\n")]);
            write_file(folder, "assets/logo.bin", &[0, 1, 2, 3]);
        },
        |folder| {
            write_files(folder, &[("src/file.rs", "one\ntwo reviewed\nthree new\n")]);
            write_file(folder, "assets/logo.bin", &[0, 1, 2, 4]);
        },
    );

    let outcome = run_scope(folder.path(), &history).expect("build the scope base");
    let Outcome::Base {
        commit: base,
        files,
        added,
        removed,
    } = outcome
    else {
        panic!("expected a base commit, got {outcome:?}");
    };

    assert_eq!((files, added, removed), (1, 1, 0));
    assert_eq!(
        changed_paths(folder.path(), &base, &history.head),
        ["src/file.rs"]
    );
    assert_eq!(
        object_id(folder.path(), &format!("{base}:assets/logo.bin")),
        object_id(folder.path(), &format!("{}:assets/logo.bin", history.head))
    );
}

#[cfg(unix)]
#[test]
fn keeps_the_head_entry_of_a_symlink() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| {
            write_files(folder, &[("src/file.rs", "one\n")]);
            write_symlink(folder, "link", "target one");
        },
        |folder| write_files(folder, &[("src/file.rs", "one\ntwo reviewed\n")]),
        |_| {},
        |folder| {
            write_files(folder, &[("src/file.rs", "one\ntwo reviewed\nthree new\n")]);
            write_symlink(folder, "link", "target three");
        },
    );

    let outcome = run_scope(folder.path(), &history).expect("build the scope base");
    let Outcome::Base {
        commit: base,
        files,
        added,
        removed,
    } = outcome
    else {
        panic!("expected a base commit, got {outcome:?}");
    };

    assert_eq!((files, added, removed), (1, 1, 0));
    assert_eq!(
        changed_paths(folder.path(), &base, &history.head),
        ["src/file.rs"]
    );
    assert_eq!(
        tree_entry(folder.path(), &base, "link"),
        tree_entry(folder.path(), &history.head, "link")
    );
}

#[test]
fn keeps_the_head_entry_of_a_submodule() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| {
            write_files(folder, &[("src/file.rs", "one\n")]);
            write_submodule(folder, "vendor/sub", "inner one\n");
        },
        |folder| write_files(folder, &[("src/file.rs", "one\ntwo reviewed\n")]),
        |_| {},
        |folder| {
            write_files(folder, &[("src/file.rs", "one\ntwo reviewed\nthree new\n")]);
            write_submodule(folder, "vendor/sub", "inner two\n");
        },
    );

    let outcome = run_scope(folder.path(), &history).expect("build the scope base");
    let Outcome::Base {
        commit: base,
        files,
        added,
        removed,
    } = outcome
    else {
        panic!("expected a base commit, got {outcome:?}");
    };

    assert_eq!((files, added, removed), (1, 1, 0));
    assert_eq!(
        changed_paths(folder.path(), &base, &history.head),
        ["src/file.rs"]
    );
    assert_eq!(
        tree_entry(folder.path(), &base, "vendor/sub"),
        tree_entry(folder.path(), &history.head, "vendor/sub")
    );
}

#[cfg(unix)]
#[test]
fn keeps_the_head_entry_of_a_mode_only_change() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| {
            write_files(
                folder,
                &[("src/file.rs", "one\n"), ("src/tool.sh", "echo one\n")],
            )
        },
        |folder| write_files(folder, &[("src/file.rs", "one\ntwo reviewed\n")]),
        |_| {},
        |folder| {
            write_files(folder, &[("src/file.rs", "one\ntwo reviewed\nthree new\n")]);
            make_executable(folder, "src/tool.sh");
        },
    );

    let outcome = run_scope(folder.path(), &history).expect("build the scope base");
    let Outcome::Base {
        commit: base,
        files,
        added,
        removed,
    } = outcome
    else {
        panic!("expected a base commit, got {outcome:?}");
    };

    assert_eq!((files, added, removed), (1, 1, 0));
    assert_eq!(
        changed_paths(folder.path(), &base, &history.head),
        ["src/file.rs"]
    );
    assert_eq!(
        tree_entry(folder.path(), &base, "src/tool.sh"),
        tree_entry(folder.path(), &history.head, "src/tool.sh")
    );
}

#[test]
fn keeps_no_final_newline_in_a_rebuilt_base_file() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| write_files(folder, &[("src/other.rs", "other\n")]),
        |folder| write_file(folder, "src/new.rs", b"new first"),
        |_| {},
        |folder| write_file(folder, "src/new.rs", b"new first\nnew second"),
    );

    let outcome = run_scope(folder.path(), &history).expect("build the scope base");
    let Outcome::Base {
        commit: base,
        files,
        added,
        removed,
    } = outcome
    else {
        panic!("expected a base commit, got {outcome:?}");
    };

    assert_eq!((files, added, removed), (1, 1, 0));
    assert_eq!(
        blob_text(folder.path(), &format!("{base}:src/new.rs")),
        "new first"
    );
    assert_eq!(
        changed_paths(folder.path(), &base, &history.head),
        ["src/new.rs"]
    );
}

#[test]
fn leaves_out_a_new_file_whose_lines_are_all_scoped() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| write_files(folder, &[("src/other.rs", "other\n")]),
        |_| {},
        |_| {},
        |folder| write_files(folder, &[("src/new.rs", "brand new one\nbrand new two\n")]),
    );

    let outcome = run_scope(folder.path(), &history).expect("build the scope base");
    let Outcome::Base {
        commit: base,
        files,
        added,
        removed,
    } = outcome
    else {
        panic!("expected a base commit, got {outcome:?}");
    };

    assert_eq!((files, added, removed), (1, 2, 0));
    assert!(tree_entry(folder.path(), &base, "src/new.rs").is_empty());
    assert_eq!(
        changed_paths(folder.path(), &base, &history.head),
        ["src/new.rs"]
    );
}

#[test]
fn keeps_an_empty_base_entry_when_every_head_line_is_scoped() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| write_files(folder, &[("src/file.rs", "old line\n")]),
        |folder| write_files(folder, &[("src/file.rs", "reviewed line\n")]),
        |folder| write_files(folder, &[("src/file.rs", "old line\n")]),
        |folder| write_files(folder, &[("src/file.rs", "new line\n")]),
    );

    let outcome = run_scope(folder.path(), &history).expect("build the scope base");
    let Outcome::Base {
        commit: base,
        files,
        added,
        removed,
    } = outcome
    else {
        panic!("expected a base commit, got {outcome:?}");
    };

    assert_eq!((files, added, removed), (1, 1, 0));
    assert_eq!(blob_text(folder.path(), &format!("{base}:src/file.rs")), "");
    assert_eq!(
        changed_paths(folder.path(), &base, &history.head),
        ["src/file.rs"]
    );
}

#[test]
fn counts_scoped_whole_files_and_line_files() {
    let mut added = BTreeSet::new();
    added.insert(1);
    added.insert(3);
    let removed = vec![InsertedLine {
        after: 1,
        text: "old".to_string(),
    }];
    let scoped = BTreeMap::from([
        (
            "src/lines.rs".to_string(),
            ScopedFile::Lines {
                added,
                removed,
                base_present: true,
            },
        ),
        (
            "src/whole.rs".to_string(),
            ScopedFile::Whole {
                base_object: None,
                base_mode: "100644".to_string(),
                added: 4,
                removed: 2,
            },
        ),
    ]);

    assert_eq!(counts(&scoped), (2, 6, 3));
}

#[test]
fn reads_ordered_raw_records_with_rename_paths() {
    let bytes = b":100644 100644 old-object new-object M\0src/a.rs\0:100644 000000 old-object zero-object D\0src/b.rs\0:100644 100755 old-object new-object R100\0src/c.rs\0src/d.rs\0";

    let changes = parse_raw_changes(bytes).expect("parse the raw records");

    assert_eq!(changes.len(), 3);
    assert_eq!(changes[0].status, 'M');
    assert_eq!(changes[0].old_path, "src/a.rs");
    assert_eq!(changes[0].new_path, "src/a.rs");
    assert_eq!(changes[0].old_mode, "100644");
    assert_eq!(changes[0].new_mode, "100644");
    assert_eq!(changes[0].old_object, "old-object");
    assert_eq!(changes[1].status, 'D');
    assert_eq!(changes[1].old_path, "src/b.rs");
    assert_eq!(changes[2].status, 'R');
    assert_eq!(changes[2].old_path, "src/c.rs");
    assert_eq!(changes[2].new_path, "src/d.rs");
    assert_eq!(changes[2].new_mode, "100755");
}

#[test]
fn reads_binary_and_text_numstat_records() {
    let stats = parse_numstat(b"3\t4\tsrc/a.rs\0-\t-\tassets/logo.bin\0")
        .expect("parse the numstat records");

    assert!(matches!(
        stats.get("src/a.rs"),
        Some(Counts::Text {
            added: 3,
            removed: 4
        })
    ));
    assert!(matches!(stats.get("assets/logo.bin"), Some(Counts::Binary)));
}

#[test]
fn falls_back_to_zero_for_an_unparsed_numstat_count() {
    let stats = parse_numstat(b"not-a-number\t2\tsrc/a.rs\0").expect("parse the numstat record");

    assert!(matches!(
        stats.get("src/a.rs"),
        Some(Counts::Text {
            added: 0,
            removed: 2
        })
    ));
}

#[test]
fn refuses_a_reviewed_head_that_is_not_in_the_clone() {
    let folder = repository_folder();
    write_files(folder.path(), &[("src/file.rs", "one\n")]);
    commit(folder.path(), "base");
    let head = head_commit(folder.path());
    let missing = "0".repeat(40);
    let request = Request {
        reviewed: missing.clone(),
        reviewed_base: head.clone(),
        base: head.clone(),
        head,
    };

    let error = run(folder.path(), &request).expect_err("refuse a missing commit");

    assert!(matches!(
        error,
        OcrScopeError::NotACommit { flag: "--reviewed", ref value } if value == &missing
    ));
    assert!(replace_refs(folder.path()).is_empty());
}

#[test]
fn gives_the_same_base_for_the_same_input() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| write_files(folder, BASE_FILES),
        |folder| write_files(folder, REVIEWED_FILES),
        |folder| write_files(folder, MOVED_BASE_FILES),
        |folder| write_files(folder, HEAD_FILES),
    );

    let first = run_scope(folder.path(), &history).expect("build the first base");
    let second = run_scope(folder.path(), &history).expect("build the second base");

    assert_eq!(first, second);
}

fn run_scope(folder: &Path, history: &History) -> Result<Outcome, OcrScopeError> {
    let request = Request {
        reviewed: history.reviewed.clone(),
        reviewed_base: history.base.clone(),
        base: history.moved_base.clone(),
        head: history.head.clone(),
    };

    run(folder, &request)
}

fn repository_folder() -> TempDir {
    let folder = tempfile::Builder::new()
        .prefix("stomme-ocr-scope-")
        .tempdir()
        .expect("create a temporary folder");
    git(folder.path(), &["init", "-q"]);

    folder
}

fn build_history(
    folder: &Path,
    base: impl FnOnce(&Path),
    reviewed: impl FnOnce(&Path),
    moved_base: impl FnOnce(&Path),
    head: impl FnOnce(&Path),
) -> History {
    base(folder);
    commit_step(folder, "base");
    let base = head_commit(folder);

    git(folder, &["checkout", "-q", "-B", "reviewed", &base]);
    reviewed(folder);
    commit_step(folder, "reviewed");
    let reviewed = head_commit(folder);

    git(folder, &["checkout", "-q", "-B", "moved-base", &base]);
    moved_base(folder);
    commit_step(folder, "moved base");
    let moved_base = head_commit(folder);

    git(folder, &["checkout", "-q", "-B", "head", &moved_base]);
    head(folder);
    commit_step(folder, "head");
    let head = head_commit(folder);

    History {
        base,
        reviewed,
        moved_base,
        head,
    }
}

fn commit_step(folder: &Path, message: &str) {
    let output = git_output(folder, &["status", "--porcelain"]);

    if output.stdout.is_empty() {
        return;
    }

    commit(folder, message);
}

fn write_files(folder: &Path, files: &[(&str, &str)]) {
    for (path, text) in files {
        write_file(folder, path, text.as_bytes());
    }
}

fn write_file(folder: &Path, path: &str, bytes: &[u8]) {
    let path = folder.join(path);
    let parent = path.parent().expect("a fixture file has a parent");
    fs::create_dir_all(parent).expect("create the fixture folder");
    fs::write(&path, bytes).expect("write the fixture file");
}

fn remove_files(folder: &Path, paths: &[&str]) {
    for path in paths {
        fs::remove_file(folder.join(path)).expect("remove the fixture file");
    }
}

fn write_submodule(folder: &Path, path: &str, text: &str) {
    let nested = folder.join(path);

    fs::create_dir_all(&nested).expect("create the submodule folder");
    git(&nested, &["init", "-q"]);
    write_file(&nested, "inner.txt", text.as_bytes());
    commit(&nested, "inner");
}

#[cfg(unix)]
fn write_symlink(folder: &Path, path: &str, target: &str) {
    use std::os::unix::fs::symlink;

    let path = folder.join(path);
    let parent = path.parent().expect("a fixture symlink has a parent");

    fs::create_dir_all(parent).expect("create the fixture folder");

    if fs::symlink_metadata(&path).is_ok() {
        fs::remove_file(&path).expect("replace the fixture symlink");
    }

    symlink(target, &path).expect("create the fixture symlink");
}

#[cfg(unix)]
fn make_executable(folder: &Path, path: &str) {
    use std::os::unix::fs::PermissionsExt as _;

    let path = folder.join(path);
    let mut permissions = fs::metadata(&path)
        .expect("read the fixture mode")
        .permissions();

    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("make the fixture file executable");
}

fn head_commit(folder: &Path) -> String {
    let output = git_output(folder, &["rev-parse", "HEAD"]);

    String::from_utf8(output.stdout)
        .expect("git prints UTF-8")
        .trim()
        .to_string()
}

fn added_texts(folder: &Path, base: &str, head: &str) -> Vec<String> {
    diff_texts(folder, base, head).0
}

fn removed_texts(folder: &Path, base: &str, head: &str) -> Vec<String> {
    diff_texts(folder, base, head).1
}

fn diff_texts(folder: &Path, base: &str, head: &str) -> (Vec<String>, Vec<String>) {
    let output = git_output(folder, &["diff", "-U0", "--find-renames", base, head]);
    let text = String::from_utf8(output.stdout).expect("the diff is UTF-8");
    let mut added = Vec::new();
    let mut removed = Vec::new();

    for line in text.lines() {
        if line.starts_with("+++") || line.starts_with("---") {
            continue;
        }

        if let Some(text) = line.strip_prefix('+') {
            added.push(text.to_string());
        } else if let Some(text) = line.strip_prefix('-') {
            removed.push(text.to_string());
        }
    }

    added.sort();
    removed.sort();

    (added, removed)
}

fn changed_paths(folder: &Path, base: &str, head: &str) -> Vec<String> {
    let output = git_output(folder, &["diff", "--name-only", "-z", base, head]);
    let mut paths: Vec<String> = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .map(|field| String::from_utf8(field.to_vec()).expect("a path is UTF-8"))
        .collect();

    paths.sort();

    paths
}

fn numstat_text(folder: &Path, base: &str, head: &str) -> String {
    let output = git_output(folder, &["diff", "--numstat", base, head]);

    String::from_utf8(output.stdout)
        .expect("the numstat output is UTF-8")
        .trim()
        .to_string()
}

fn replace_refs(folder: &Path) -> String {
    let output = git_output(folder, &["replace", "-l"]);

    String::from_utf8(output.stdout)
        .expect("the replace list is UTF-8")
        .trim()
        .to_string()
}

fn tree_entry(folder: &Path, revision: &str, path: &str) -> String {
    let output = git_output(folder, &["ls-tree", revision, "--", path]);

    String::from_utf8(output.stdout)
        .expect("the tree entry is UTF-8")
        .trim()
        .to_string()
}

fn blob_text(folder: &Path, revision: &str) -> String {
    let output = git_output(folder, &["cat-file", "-p", revision]);

    String::from_utf8(output.stdout).expect("the blob is UTF-8")
}

fn object_id(folder: &Path, revision: &str) -> String {
    let output = git_output(folder, &["rev-parse", revision]);

    String::from_utf8(output.stdout)
        .expect("the object identifier is UTF-8")
        .trim()
        .to_string()
}

fn git_output(folder: &Path, arguments: &[&str]) -> std::process::Output {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(folder)
        .output()
        .expect("run git");
    assert!(output.status.success(), "{arguments:?}: {output:?}");

    output
}
