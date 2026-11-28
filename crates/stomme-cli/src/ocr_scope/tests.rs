use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

use super::git::git_text;
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
fn scopes_a_removal_the_moved_base_added() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| write_files(folder, &[("src/a.txt", "keep\n")]),
        |_| {},
        |folder| write_files(folder, &[("src/a.txt", "keep\nbase-only\n")]),
        |folder| write_files(folder, &[("src/a.txt", "keep\n")]),
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
        ["base-only"]
    );
    assert_eq!(
        changed_paths(folder.path(), &base, &history.head),
        ["src/a.txt"]
    );
}

#[test]
fn leaves_out_a_removal_the_review_already_covered() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| write_files(folder, &[("src/a.txt", "keep\nremove me\nlast\n")]),
        |folder| write_files(folder, &[("src/a.txt", "keep\nlast\n")]),
        |folder| write_files(folder, &[("src/a.txt", "keep\nremove me\nlast\n")]),
        |folder| write_files(folder, &[("src/a.txt", "keep\nlast\n")]),
    );

    let outcome = run_scope(folder.path(), &history).expect("build the scope base");

    assert_eq!(outcome, Outcome::Empty);
    assert!(replace_refs(folder.path()).is_empty());
}

#[cfg(unix)]
#[test]
fn scopes_a_deletion_after_a_prior_symlink_conversion() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| write_symlink(folder, "src/a.txt", "token"),
        |folder| {
            remove_files(folder, &["src/a.txt"]);
            write_files(folder, &[("src/a.txt", "keep\ntoken\n")]);
        },
        |folder| {
            remove_files(folder, &["src/a.txt"]);
            write_files(folder, &[("src/a.txt", "keep\ntoken\n")]);
        },
        |folder| write_files(folder, &[("src/a.txt", "keep\n")]),
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
    assert_eq!(
        removed_texts(folder.path(), &base, &history.head),
        ["token"]
    );
}

#[cfg(unix)]
#[test]
fn scopes_a_deletion_after_a_prior_regular_file_conversion() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| write_files(folder, &[("src/a.txt", "keep\ntoken\nlast\n")]),
        |folder| {
            remove_files(folder, &["src/a.txt"]);
            write_symlink(folder, "src/a.txt", "token");
        },
        |folder| write_files(folder, &[("src/a.txt", "keep\ntoken\nlast\n")]),
        |folder| write_files(folder, &[("src/a.txt", "keep\nlast\n")]),
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
    assert_eq!(
        removed_texts(folder.path(), &base, &history.head),
        ["token"]
    );
}

#[test]
fn gives_the_same_scope_from_a_nested_folder() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| write_files(folder, BASE_FILES),
        |folder| write_files(folder, REVIEWED_FILES),
        |folder| write_files(folder, MOVED_BASE_FILES),
        |folder| write_files(folder, HEAD_FILES),
    );

    let from_root = run_scope(folder.path(), &history).expect("build the scope from the root");
    let from_nested = run_scope(&folder.path().join("src"), &history)
        .expect("build the scope from a nested folder");

    assert!(matches!(from_root, Outcome::Base { .. }));
    assert_eq!(from_nested, from_root);
}

#[cfg(unix)]
#[test]
fn builds_the_scope_from_a_root_that_ends_in_a_space() {
    let parent = tempfile::Builder::new()
        .prefix("stomme-ocr-scope-space-")
        .tempdir()
        .expect("create a temporary folder");
    let folder = parent.path().join("root ");

    fs::create_dir(&folder).expect("create the repository folder");
    git(&folder, &["init", "-q"]);

    let history = build_history(
        &folder,
        |folder| write_files(folder, &[("src/a.txt", "keep\n")]),
        |folder| write_files(folder, &[("src/a.txt", "keep\nreviewed\n")]),
        |folder| write_files(folder, &[("src/a.txt", "keep\nreviewed\n")]),
        |folder| write_files(folder, &[("src/a.txt", "keep\nreviewed\nnew\n")]),
    );

    let outcome = run_scope(&folder, &history).expect("build the scope base");
    let Outcome::Base {
        files,
        added,
        removed,
        ..
    } = outcome
    else {
        panic!("expected a base commit, got {outcome:?}");
    };

    assert_eq!((files, added, removed), (1, 1, 0));
}

#[cfg(unix)]
#[test]
fn skips_a_head_path_that_is_not_utf8() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt as _;

    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| {
            write_files(folder, &[("src/file.rs", "one\n")]);
            let path = folder.join(OsStr::from_bytes(b"src/bad-\xff.rs"));

            fs::write(path, "x\n").expect("write the fixture file");
        },
        |_| {},
        |_| {},
        |folder| write_file(folder, "src/file.rs", b"one\ntwo\n"),
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

    let output = git_output(
        folder.path(),
        &["diff", "--name-only", &base, &history.head],
    );

    assert_eq!(output.stdout, b"src/file.rs\n");
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

#[cfg(unix)]
#[test]
fn keeps_the_head_entry_when_a_symlink_becomes_a_regular_file() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| {
            write_files(folder, &[("src/file.rs", "one\n")]);
            write_symlink(folder, "src/a.txt", "target");
        },
        |folder| write_files(folder, &[("src/file.rs", "one\ntwo reviewed\n")]),
        |folder| write_files(folder, &[("src/file.rs", "one\n")]),
        |folder| {
            remove_files(folder, &["src/a.txt"]);
            write_files(
                folder,
                &[
                    ("src/file.rs", "one\ntwo reviewed\nthree new\n"),
                    ("src/a.txt", "new\n"),
                ],
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
        changed_paths(folder.path(), &base, &history.head),
        ["src/file.rs"]
    );
    assert_eq!(
        tree_entry(folder.path(), &base, "src/a.txt"),
        tree_entry(folder.path(), &history.head, "src/a.txt")
    );
}

#[cfg(unix)]
#[test]
fn keeps_the_head_entry_when_a_regular_file_becomes_a_symlink() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| {
            write_files(
                folder,
                &[("src/file.rs", "one\n"), ("src/a.txt", "target\n")],
            );
        },
        |folder| write_files(folder, &[("src/file.rs", "one\ntwo reviewed\n")]),
        |folder| write_files(folder, &[("src/file.rs", "one\n")]),
        |folder| {
            write_files(folder, &[("src/file.rs", "one\ntwo reviewed\nthree new\n")]);
            write_symlink(folder, "src/a.txt", "target");
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
        tree_entry(folder.path(), &base, "src/a.txt"),
        tree_entry(folder.path(), &history.head, "src/a.txt")
    );
}

#[cfg(unix)]
#[test]
fn scopes_a_new_line_after_a_reviewed_symlink_becomes_regular() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| write_files(folder, &[("src/a.txt", "keep\n")]),
        |folder| {
            remove_files(folder, &["src/a.txt"]);
            write_symlink(folder, "src/a.txt", "token");
        },
        |_| {},
        |folder| write_files(folder, &[("src/a.txt", "new\nkeep\n")]),
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
    assert_eq!(added_texts(folder.path(), &base, &history.head), ["new"]);
}

#[test]
fn scopes_a_new_line_after_a_reviewed_submodule_becomes_regular() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| write_files(folder, &[("src/a.txt", "keep\n")]),
        |folder| {
            remove_files(folder, &["src/a.txt"]);
            write_submodule(folder, "src/a.txt", "inner one\n");
        },
        |_| {},
        |folder| write_files(folder, &[("src/a.txt", "new\nkeep\n")]),
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
    assert_eq!(added_texts(folder.path(), &base, &history.head), ["new"]);
}

#[test]
fn keeps_the_head_entry_when_a_submodule_becomes_a_regular_file() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| {
            write_files(folder, &[("src/file.rs", "one\n")]);
            write_submodule(folder, "vendor/sub", "inner one\n");
        },
        |folder| write_files(folder, &[("src/file.rs", "one\ntwo reviewed\n")]),
        |folder| write_files(folder, &[("src/file.rs", "one\n")]),
        |folder| {
            remove_folder(folder, "vendor/sub");
            write_files(
                folder,
                &[
                    ("src/file.rs", "one\ntwo reviewed\nthree new\n"),
                    ("vendor/sub", "new\n"),
                ],
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
        tree_entry(folder.path(), &base, "vendor/sub"),
        tree_entry(folder.path(), &history.head, "vendor/sub")
    );
}

#[test]
fn keeps_the_bytes_of_a_restored_latin1_line() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| write_file(folder, "src/a.txt", b"keep\nlatin-1 \xe9\n"),
        |_| {},
        |_| {},
        |folder| write_file(folder, "src/a.txt", b"keep\n"),
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
    assert_eq!(
        blob_bytes(folder.path(), &format!("{base}:src/a.txt")),
        b"keep\nlatin-1 \xe9\n"
    );
}

#[test]
fn keeps_an_unterminated_removed_line_unterminated() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| write_file(folder, "src/a.txt", b"keep\nlast"),
        |_| {},
        |_| {},
        |folder| write_file(folder, "src/a.txt", b"keep\n"),
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
    assert_eq!(
        blob_bytes(folder.path(), &format!("{base}:src/a.txt")),
        b"keep\nlast"
    );
}

#[test]
fn writes_a_new_file_whose_name_starts_with_a_dash() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| write_files(folder, &[("src/file.rs", "one\n")]),
        |_| {},
        |_| {},
        |folder| write_files(folder, &[("-note.txt", "new\n")]),
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
    assert!(tree_entry(folder.path(), &base, "-note.txt").is_empty());
    assert_eq!(
        changed_paths(folder.path(), &base, &history.head),
        ["-note.txt"]
    );
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
        |folder| write_file(folder, "src/new.rs", b"new first\nnew second"),
        |_| {},
        |folder| write_file(folder, "src/new.rs", b"new first\nnew second\nnew third"),
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
    // `rebuild` keeps the newline that follows every retained head line, so the last
    // retained line stays terminated when the added line after it is dropped.
    assert_eq!(
        blob_bytes(folder.path(), &format!("{base}:src/new.rs")),
        b"new first\nnew second\n"
    );
    assert_eq!(
        changed_paths(folder.path(), &base, &history.head),
        ["src/new.rs"]
    );
}

#[test]
fn keeps_the_terminator_of_the_last_retained_head_line() {
    let folder = repository_folder();
    let history = build_history(
        folder.path(),
        |folder| write_file(folder, "src/file.rs", b"a\n"),
        |_| {},
        |_| {},
        |folder| write_file(folder, "src/file.rs", b"a\nb"),
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
        blob_bytes(folder.path(), &format!("{base}:src/file.rs")),
        b"a\n"
    );
    assert_eq!(added_texts(folder.path(), &base, &history.head), ["b"]);
    assert!(removed_texts(folder.path(), &base, &history.head).is_empty());
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
fn reports_a_scoped_path_missing_from_the_head_tree() {
    let folder = repository_folder();
    let git = Git {
        root: folder.path(),
    };
    let scoped = BTreeMap::from([(
        "src/file.rs".to_string(),
        ScopedFile::Lines {
            added: BTreeSet::from([1]),
            removed: Vec::new(),
            base_present: true,
        },
    )]);

    let error = build_base_commit(&git, "HEAD", &BTreeMap::new(), &scoped)
        .expect_err("refuse a path that the head tree does not hold");

    assert!(matches!(
        error,
        OcrScopeError::MissingHeadEntry { ref path } if path == "src/file.rs"
    ));
}

#[test]
fn reports_the_failed_git_command_and_its_stderr() {
    let folder = tempfile::Builder::new()
        .prefix("stomme-ocr-scope-broken-repository-")
        .tempdir()
        .expect("create a temporary folder");
    fs::write(folder.path().join(".git"), "garbage\n").expect("write a broken git file");
    let request = Request {
        reviewed: "HEAD".to_string(),
        reviewed_base: "HEAD".to_string(),
        base: "HEAD".to_string(),
        head: "HEAD".to_string(),
    };

    let error = run(folder.path(), &request).expect_err("refuse a folder with a broken repository");

    let OcrScopeError::Git { command, stderr } = error else {
        panic!("expected a git error, got {error:?}");
    };
    let output = Command::new("git")
        .args(["rev-parse", "--is-shallow-repository"])
        .current_dir(folder.path())
        .output()
        .expect("run git");

    assert_eq!(command, "rev-parse --is-shallow-repository");
    assert!(!output.status.success());
    assert_eq!(stderr, String::from_utf8_lossy(&output.stderr));
}

#[test]
fn trims_the_stdout_of_a_git_command() {
    let folder = repository_folder();
    let git = Git {
        root: folder.path(),
    };

    let text =
        git_text(&git, &["rev-parse", "--is-shallow-repository"]).expect("read the shallow state");

    assert_eq!(text, "false");
}

#[test]
fn refuses_a_shallow_clone() {
    let source = repository_folder();
    write_files(source.path(), &[("src/file.rs", "one\n")]);
    commit(source.path(), "base");
    write_files(source.path(), &[("src/file.rs", "one\ntwo\n")]);
    commit(source.path(), "head");

    let folder = tempfile::Builder::new()
        .prefix("stomme-ocr-scope-shallow-")
        .tempdir()
        .expect("create a temporary folder");
    let url = format!("file://{}", source.path().display());
    git(
        folder.path(),
        &["clone", "-q", "--depth", "1", url.as_str(), "."],
    );

    let shallow = git_output(folder.path(), &["rev-parse", "--is-shallow-repository"]);
    assert_eq!(shallow.stdout, b"true\n");

    let head = head_commit(folder.path());
    let request = Request {
        reviewed: head.clone(),
        reviewed_base: head.clone(),
        base: head.clone(),
        head,
    };

    let error = run(folder.path(), &request).expect_err("refuse a shallow clone");

    assert!(matches!(error, OcrScopeError::ShallowClone));
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

fn remove_folder(folder: &Path, path: &str) {
    fs::remove_dir_all(folder.join(path)).expect("remove the fixture folder");
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
    let mut in_file_header = false;

    for line in text.lines() {
        if line.starts_with("diff --git ") {
            in_file_header = true;
            continue;
        }

        if line.starts_with("@@") {
            in_file_header = false;
            continue;
        }

        if in_file_header {
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

fn blob_bytes(folder: &Path, revision: &str) -> Vec<u8> {
    git_output(folder, &["cat-file", "-p", revision]).stdout
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
