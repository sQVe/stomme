use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use super::*;
use crate::git_isolation::REPOSITORY_VARIABLES;
use crate::test_support::{strings, temporary_directory};

fn ocr(root: &Path, home: &Path) -> Ocr {
    Ocr {
        binary: root.join(OCR_BINARY),
        root: root.to_path_buf(),
        home: home.to_path_buf(),
    }
}

fn rules_without_include(source: &Path, folder: &Path) -> PathBuf {
    let text = fs::read_to_string(source).expect("read the rule file");
    let mut value: serde_json::Value = serde_json::from_str(&text).expect("parse the rule file");
    let object = value.as_object_mut().expect("the rule file is an object");
    object.remove("include");
    let path = folder.join("without-include.json");
    let rendered = serde_json::to_string(&value).expect("render the rule file");
    fs::write(&path, rendered).expect("write the rule file");

    path
}

fn rules_with_replaced_patterns(
    source: &Path,
    folder: &Path,
    replacements: &[(&str, &str)],
) -> PathBuf {
    let text = fs::read_to_string(source).expect("read the rule file");
    let mut value: serde_json::Value = serde_json::from_str(&text).expect("parse the rule file");
    let rules = value
        .get_mut("rules")
        .and_then(|rules| rules.as_array_mut())
        .expect("the rule file has rules");

    for (path, pattern) in replacements {
        let rule = rules
            .iter_mut()
            .find(|rule| rule["path"].as_str() == Some(*path))
            .unwrap_or_else(|| panic!("no rule with path {path}"));

        rule["path"] = serde_json::Value::String((*pattern).to_string());
    }

    let path = folder.join("replaced-patterns.json");
    let rendered = serde_json::to_string(&value).expect("render the rule file");
    fs::write(&path, rendered).expect("write the rule file");

    path
}

fn run_repository_case(
    root: &Path,
    home: &Path,
    repository: &Repository,
    rules: &Path,
) -> CaseOutcome {
    run_case(
        &ocr(root, home),
        repository.folder.path(),
        rules,
        &repository.base,
        &repository.head,
        &REPOSITORY_CASE,
    )
    .expect("run the repository case")
}

fn environment(command: &Command, name: &str) -> Option<String> {
    let entry = command
        .get_envs()
        .find(|(key, _)| *key == OsStr::new(name))?;
    let value = entry.1?;

    Some(value.to_string_lossy().into_owned())
}

fn arguments(command: &Command) -> Vec<String> {
    command
        .get_args()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn repository_case_passes_on_the_current_rules() {
    let root = crate::workspace_root();
    let home = temporary_directory();
    let repository = build_repository().expect("build the pinned diff");

    let outcome = run_repository_case(&root, home.path(), &repository, &root.join(RULES));

    assert_eq!(outcome.admission, Vec::<String>::new());
    assert_eq!(outcome.precedence, Some(Vec::<String>::new()));
}

#[test]
fn repository_case_fails_admission_without_include() {
    let root = crate::workspace_root();
    let home = temporary_directory();
    let repository = build_repository().expect("build the pinned diff");
    let rules = rules_without_include(&root.join(RULES), home.path());

    let outcome = run_repository_case(&root, home.path(), &repository, &rules);

    assert_eq!(
        outcome.admission,
        [
            "ocr-smoke: repository: admission failed: crates/demo/tests/add.rs expected reviewed, got excluded (default_path)",
            "ocr-smoke: repository: admission failed: packages/demo/tests/add.test.ts expected reviewed, got excluded (default_path)",
            "ocr-smoke: repository: admission failed: packages/demo/src/add.test.ts expected reviewed, got excluded (default_path)"
        ]
    );
    assert_eq!(outcome.precedence, Some(Vec::<String>::new()));
}

#[test]
fn repository_case_fails_precedence_on_a_catch_all_language_rule() {
    let root = crate::workspace_root();
    let home = temporary_directory();
    let repository = build_repository().expect("build the pinned diff");
    let rules =
        rules_with_replaced_patterns(&root.join(RULES), home.path(), &[("**/*.rs", "**/*")]);

    let outcome = run_repository_case(&root, home.path(), &repository, &rules);

    assert_eq!(outcome.admission, Vec::<String>::new());
    assert_eq!(
        outcome.precedence,
        Some(strings(&[
            "ocr-smoke: repository: precedence failed: crates/demo/tests/add.rs expected Custom (--rule) pattern **/*.rs, got Custom (--rule) pattern **/*"
        ]))
    );
}

#[test]
fn repository_case_fails_precedence_on_a_wrong_package_rule() {
    let root = crate::workspace_root();
    let home = temporary_directory();
    let repository = build_repository().expect("build the pinned diff");
    let rules = rules_with_replaced_patterns(
        &root.join(RULES),
        home.path(),
        &[(
            "packages/**/*.{ts,tsx,js,jsx,mjs,cjs,mts,cts}",
            "packages/**/*.ts",
        )],
    );

    let outcome = run_repository_case(&root, home.path(), &repository, &rules);

    assert_eq!(outcome.admission, Vec::<String>::new());
    assert_eq!(
        outcome.precedence,
        Some(strings(&[
            "ocr-smoke: repository: precedence failed: packages/demo/tests/add.test.ts expected Custom (--rule) pattern packages/**/*.{ts,tsx,js,jsx,mjs,cjs,mts,cts}, got Custom (--rule) pattern packages/**/*.ts",
            "ocr-smoke: repository: precedence failed: packages/demo/src/add.test.ts expected Custom (--rule) pattern packages/**/*.{ts,tsx,js,jsx,mjs,cjs,mts,cts}, got Custom (--rule) pattern packages/**/*.ts"
        ]))
    );
}

#[test]
fn repository_case_reports_both_wrong_precedence_rows() {
    let root = crate::workspace_root();
    let home = temporary_directory();
    let repository = build_repository().expect("build the pinned diff");
    let rules = rules_with_replaced_patterns(
        &root.join(RULES),
        home.path(),
        &[
            ("**/*.rs", "**/*"),
            (
                "packages/**/*.{ts,tsx,js,jsx,mjs,cjs,mts,cts}",
                "packages/**/*.ts",
            ),
        ],
    );

    let outcome = run_repository_case(&root, home.path(), &repository, &rules);

    assert_eq!(outcome.admission, Vec::<String>::new());
    assert_eq!(
        outcome.precedence,
        Some(strings(&[
            "ocr-smoke: repository: precedence failed: crates/demo/tests/add.rs expected Custom (--rule) pattern **/*.rs, got Custom (--rule) pattern **/*",
            "ocr-smoke: repository: precedence failed: packages/demo/tests/add.test.ts expected Custom (--rule) pattern packages/**/*.{ts,tsx,js,jsx,mjs,cjs,mts,cts}, got Custom (--rule) pattern packages/**/*.ts",
            "ocr-smoke: repository: precedence failed: packages/demo/src/add.test.ts expected Custom (--rule) pattern packages/**/*.{ts,tsx,js,jsx,mjs,cjs,mts,cts}, got Custom (--rule) pattern packages/**/*.ts"
        ]))
    );
}

#[test]
fn typescript_consumer_case_passes_and_fails_without_include() {
    let root = crate::workspace_root();
    let home = temporary_directory();
    let repository = build_repository().expect("build the pinned diff");
    let consumer = build_typescript_consumer(&root).expect("build the TypeScript consumer");
    let rules = consumer.path().join(RULES);

    let outcome = run_case(
        &ocr(&root, home.path()),
        repository.folder.path(),
        &rules,
        &repository.base,
        &repository.head,
        &TYPESCRIPT_CASE,
    )
    .expect("run the TypeScript consumer case");

    assert_eq!(outcome.admission, Vec::<String>::new());
    assert_eq!(outcome.precedence, Some(Vec::<String>::new()));

    let without_include = rules_without_include(&rules, home.path());
    let failing = run_case(
        &ocr(&root, home.path()),
        repository.folder.path(),
        &without_include,
        &repository.base,
        &repository.head,
        &TYPESCRIPT_CASE,
    )
    .expect("run the TypeScript consumer case without include");

    assert_eq!(
        failing.admission,
        [
            "ocr-smoke: typescript consumer: admission failed: packages/demo/tests/add.test.ts expected reviewed, got excluded (default_path)"
        ]
    );
    assert_eq!(failing.precedence, Some(Vec::<String>::new()));
}

#[test]
fn check_passes_on_the_current_tree() {
    let outcomes = check(&crate::workspace_root()).expect("run the smoke check");
    let expected = [
        ("repository", true),
        ("typescript consumer", true),
        ("scope", false),
    ];

    assert_eq!(outcomes.len(), expected.len());

    for (outcome, (name, checks_precedence)) in outcomes.iter().zip(expected) {
        assert_eq!(outcome.name, name);
        assert!(outcome.admission.is_empty(), "{name}");

        match (checks_precedence, outcome.precedence.as_deref()) {
            (true, Some(precedence)) => assert!(precedence.is_empty(), "{name}"),
            (false, None) => {}
            _ => panic!("{name} has the wrong precedence check"),
        }
    }
}

#[test]
fn version_failure_reports_every_mismatch() {
    let mismatch = Some(
        "node_modules/.bin/ocr is version 1.14.1, but package.json pins 1.14.2; run pnpm install --frozen-lockfile"
            .to_string(),
    );
    let cases = [
        ("the pinned release", "1.14.2", None),
        ("another release", "1.14.1", mismatch),
    ];

    for (name, installed, expected) in cases {
        assert_eq!(version_failure(installed, "1.14.2"), expected, "{name}");
    }
}

#[test]
fn version_in_reads_the_first_version_token() {
    let cases = [
        (
            "a version banner",
            "open-code-review v1.14.2 (e74f2c6) linux/amd64\nbuilt at: today",
            Some("1.14.2".to_string()),
        ),
        ("a banner without a version", "open-code-review\n", None),
    ];

    for (name, text, expected) in cases {
        assert_eq!(version_in(text), expected, "{name}");
    }
}

#[test]
fn pinned_ocr_version_reads_the_manifest() {
    let root = temporary_directory();

    fs::write(
        root.path().join(PACKAGE_JSON),
        format!(r#"{{"devDependencies":{{"{OCR_PACKAGE}":"9.8.7"}}}}"#),
    )
    .expect("write the manifest");

    let version = pinned_ocr_version(root.path()).expect("read the pinned version");

    assert_eq!(version, "9.8.7");
}

#[cfg(unix)]
#[test]
fn check_refuses_an_installed_ocr_that_differs_from_the_pin() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = temporary_directory();
    let binary = root.path().join(OCR_BINARY);
    let parent = binary.parent().expect("the binary has a parent");

    fs::create_dir_all(parent).expect("create the binary folder");
    fs::write(
        &binary,
        "#!/bin/sh\necho 'open-code-review v9.9.9 (deadbee) linux/amd64'\n",
    )
    .expect("write the fake OCR binary");
    let mut permissions = fs::metadata(&binary)
        .expect("read the fake binary mode")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&binary, permissions).expect("make the fake binary executable");
    fs::write(
        root.path().join(PACKAGE_JSON),
        format!(r#"{{"devDependencies":{{"{OCR_PACKAGE}":"1.14.2"}}}}"#),
    )
    .expect("write the manifest");

    let Err(error) = check(root.path()) else {
        panic!("expected the version check to refuse another release");
    };
    let message = format!("{error:#}");

    assert!(message.contains("is version 9.9.9"), "{message}");
    assert!(message.contains("pins 1.14.2"), "{message}");
}

#[test]
fn precedence_fails_on_a_system_built_in_source() {
    let checked = RuleCheck {
        source: "System built-in".to_string(),
        pattern: "**/*.rs".to_string(),
    };

    let failure = precedence_failure(
        "repository",
        "crates/demo/tests/add.rs",
        &checked,
        "**/*.rs",
    );

    assert_eq!(
        failure,
        Some(
            "ocr-smoke: repository: precedence failed: crates/demo/tests/add.rs expected Custom (--rule) pattern **/*.rs, got System built-in pattern **/*.rs"
                .to_string()
        )
    );
}

#[test]
fn precedence_fails_on_a_wrong_pattern() {
    let checked = RuleCheck {
        source: CUSTOM_RULE_SOURCE.to_string(),
        pattern: "**/*.ts".to_string(),
    };

    let failure = precedence_failure(
        "repository",
        "crates/demo/tests/add.rs",
        &checked,
        "**/*.rs",
    );

    assert_eq!(
        failure,
        Some(
            "ocr-smoke: repository: precedence failed: crates/demo/tests/add.rs expected Custom (--rule) pattern **/*.rs, got Custom (--rule) pattern **/*.ts"
                .to_string()
        )
    );
}

#[test]
fn precedence_passes_on_the_language_rule() {
    let checked = RuleCheck {
        source: CUSTOM_RULE_SOURCE.to_string(),
        pattern: "**/*.rs".to_string(),
    };

    assert_eq!(
        precedence_failure(
            "repository",
            "crates/demo/tests/add.rs",
            &checked,
            "**/*.rs"
        ),
        None
    );
}

#[test]
fn admission_fails_on_an_excluded_path_with_the_wrong_reason() {
    let preview = Preview {
        files: vec![PreviewFile {
            path: "crates/demo/tests/add.rs".to_string(),
            will_review: false,
            exclude_reason: Some("default_path".to_string()),
        }],
    };

    let failures = admission_failures(
        "repository",
        &preview,
        &[],
        &[("crates/demo/tests/add.rs", "user_exclude")],
    );

    assert_eq!(
        failures,
        [
            "ocr-smoke: repository: admission failed: crates/demo/tests/add.rs expected excluded (user_exclude), got excluded (default_path)"
        ]
    );
}

#[test]
fn admission_fails_on_an_excluded_path_that_is_reviewed() {
    let preview = Preview {
        files: vec![PreviewFile {
            path: "crates/demo/tests/add.rs".to_string(),
            will_review: true,
            exclude_reason: None,
        }],
    };

    let failures = admission_failures(
        "repository",
        &preview,
        &[],
        &[("crates/demo/tests/add.rs", "user_exclude")],
    );

    assert_eq!(
        failures,
        [
            "ocr-smoke: repository: admission failed: crates/demo/tests/add.rs expected excluded (user_exclude), got reviewed"
        ]
    );
}

#[test]
fn admission_fails_on_a_missing_path() {
    let preview = Preview { files: Vec::new() };

    let failures = admission_failures(
        "repository",
        &preview,
        &["crates/demo/src/lib.rs"],
        &[("crates/demo/tests/add.rs", "user_exclude")],
    );

    assert_eq!(
        failures,
        [
            "ocr-smoke: repository: admission failed: crates/demo/src/lib.rs not in the preview",
            "ocr-smoke: repository: admission failed: crates/demo/tests/add.rs not in the preview",
        ]
    );
}

#[test]
fn admission_fails_on_a_reviewed_path_that_is_excluded() {
    let preview = Preview {
        files: vec![PreviewFile {
            path: "crates/demo/tests/add.rs".to_string(),
            will_review: false,
            exclude_reason: Some("default_path".to_string()),
        }],
    };

    let failures = admission_failures("repository", &preview, &["crates/demo/tests/add.rs"], &[]);

    assert_eq!(
        failures,
        [
            "ocr-smoke: repository: admission failed: crates/demo/tests/add.rs expected reviewed, got excluded (default_path)"
        ]
    );
}

#[test]
fn parse_preview_rejects_json_without_files() {
    let error = parse_preview("{}", "").expect_err("a preview without files");

    assert!(
        format!("{error:#}").contains("missing field `files`"),
        "{error:#}"
    );
}

#[test]
fn parse_preview_rejects_a_file_without_will_review() {
    let error = parse_preview(r#"{"files":[{"path":"a.rs"}]}"#, "")
        .expect_err("a preview file without will_review");

    assert!(
        format!("{error:#}").contains("missing field `will_review`"),
        "{error:#}"
    );
}

#[test]
fn parse_preview_reports_the_command_stderr() {
    let error = parse_preview("not json", "provider warning").expect_err("unreadable JSON");

    assert!(
        format!("{error:#}").contains("provider warning"),
        "{error:#}"
    );
}

#[test]
fn parse_rule_check_rejects_output_without_a_pattern() {
    let error = parse_rule_check("File: a.rs\nSource: Custom (--rule)\n", "")
        .expect_err("a missing pattern");

    assert!(format!("{error:#}").contains("Pattern"), "{error:#}");
}

#[test]
fn parse_rule_check_reports_the_command_stderr() {
    let error = parse_rule_check("", "rule warning").expect_err("unreadable output");

    assert!(format!("{error:#}").contains("rule warning"), "{error:#}");
}

#[test]
fn ocr_command_disables_updates_and_user_configuration() {
    let home = temporary_directory();
    let command = ocr_command(
        &ocr(Path::new("/root"), home.path()),
        &["review".to_string()],
    );

    assert_eq!(environment(&command, "OCR_NO_UPDATE").as_deref(), Some("1"));
    assert_eq!(
        environment(&command, "HOME"),
        Some(home.path().display().to_string())
    );
    assert_eq!(
        environment(&command, "XDG_CONFIG_HOME"),
        Some(home.path().display().to_string())
    );
    assert_eq!(arguments(&command), ["--color", "never", "review"]);
    assert_eq!(command.get_current_dir(), Some(Path::new("/root")));
    assert_eq!(command.get_program(), "/root/node_modules/.bin/ocr");
}

#[test]
fn preview_arguments_request_the_preview_json() {
    let arguments = preview_arguments(
        Path::new("/scratch"),
        Path::new("/rules.json"),
        "base",
        "head",
    );

    assert_eq!(
        arguments,
        [
            "review",
            "--repo",
            "/scratch",
            "--rule",
            "/rules.json",
            "--from",
            "base",
            "--to",
            "head",
            "--preview",
            "--format",
            "json",
        ]
    );
}

#[test]
fn rules_check_arguments_request_the_path() {
    let arguments = rules_check_arguments(
        Path::new("/scratch"),
        Path::new("/rules.json"),
        "crates/demo/tests/add.rs",
    );

    assert_eq!(
        arguments,
        [
            "rules",
            "check",
            "--repo",
            "/scratch",
            "--rule",
            "/rules.json",
            "crates/demo/tests/add.rs",
        ]
    );
}

#[test]
fn repository_variables_cover_gits_local_variables() {
    let output = Command::new("git")
        .args(["rev-parse", "--local-env-vars"])
        .output()
        .expect("run git rev-parse");

    assert!(output.status.success());

    let reported = String::from_utf8(output.stdout).expect("git printed UTF-8");
    for name in reported.lines() {
        assert!(
            REPOSITORY_VARIABLES.contains(&name),
            "{name} is not cleared"
        );
    }
}

#[test]
fn stomme_command_runs_the_subcommand() {
    let root = Path::new("/root");
    let folder = Path::new("/scratch");
    let sync = stomme_command(root, folder, "sync");
    let scope = stomme_command(root, folder, "ocr-scope");

    assert_eq!(
        arguments(&sync),
        [
            "run",
            "--locked",
            "--quiet",
            "--manifest-path",
            "/root/Cargo.toml",
            "-p",
            "stomme",
            "--",
            "sync",
        ]
    );
    assert_eq!(
        arguments(&scope),
        [
            "run",
            "--locked",
            "--quiet",
            "--manifest-path",
            "/root/Cargo.toml",
            "-p",
            "stomme",
            "--",
            "ocr-scope",
        ]
    );
    assert_eq!(sync.get_current_dir(), Some(folder));
    assert_eq!(scope.get_current_dir(), Some(folder));
}

#[test]
fn stage_tree_stages_the_written_files() {
    let folder = temporary_directory();

    init_repository(folder.path()).expect("init the repository");
    let tree = stage_tree(
        folder.path(),
        &[("a.txt", "one\n"), ("nested/b.txt", "two\n")],
    )
    .expect("stage the files");
    let listing =
        git_text(folder.path(), &["ls-tree", "-r", "--name-only", &tree]).expect("list the tree");

    assert_eq!(listing, "a.txt\nnested/b.txt");
}

#[test]
fn scratch_commands_clear_repository_variables() {
    let home = temporary_directory();
    let commands = [
        git_command(Path::new("/scratch")),
        ocr_command(
            &ocr(Path::new("/root"), home.path()),
            &["review".to_string()],
        ),
        stomme_command(Path::new("/root"), Path::new("/scratch"), "sync"),
        stomme_command(Path::new("/root"), Path::new("/scratch"), "ocr-scope"),
    ];

    for command in commands {
        for name in REPOSITORY_VARIABLES {
            let entry = command.get_envs().find(|(key, _)| *key == OsStr::new(name));

            assert_eq!(entry, Some((OsStr::new(name), None)), "{name}");
        }

        assert_eq!(
            environment(&command, "GIT_CONFIG_GLOBAL").as_deref(),
            Some("/dev/null")
        );
        assert_eq!(
            environment(&command, "GIT_CONFIG_NOSYSTEM").as_deref(),
            Some("1")
        );
    }
}

#[test]
fn exit_code_fails_on_a_failing_assertion() {
    let passing = CaseOutcome {
        name: "case",
        admission: Vec::new(),
        precedence: Some(Vec::new()),
    };
    let failing_admission = CaseOutcome {
        name: "case",
        admission: vec!["failure".to_string()],
        precedence: Some(Vec::new()),
    };
    let failing_precedence = CaseOutcome {
        name: "case",
        admission: Vec::new(),
        precedence: Some(vec!["failure".to_string()]),
    };

    assert_eq!(exit_code(&[passing]), ExitCode::SUCCESS);
    assert_eq!(exit_code(&[failing_admission]), ExitCode::FAILURE);
    assert_eq!(exit_code(&[failing_precedence]), ExitCode::FAILURE);
}

#[test]
fn report_fails_when_an_assertion_fails() {
    let passing = CaseOutcome {
        name: "case",
        admission: Vec::new(),
        precedence: Some(Vec::new()),
    };
    let failing_admission = CaseOutcome {
        name: "case",
        admission: vec!["failure".to_string()],
        precedence: Some(Vec::new()),
    };
    let failing_precedence = CaseOutcome {
        name: "case",
        admission: Vec::new(),
        precedence: Some(vec!["failure".to_string()]),
    };

    assert!(report(std::slice::from_ref(&passing)));
    assert!(!report(std::slice::from_ref(&failing_admission)));
    assert!(!report(std::slice::from_ref(&failing_precedence)));
}

#[test]
fn report_passes_a_case_without_a_precedence_check() {
    let outcome = CaseOutcome {
        name: "scope",
        admission: Vec::new(),
        precedence: None,
    };

    assert!(report(std::slice::from_ref(&outcome)));
}

#[test]
fn run_rejects_arguments() {
    let root = crate::workspace_root();

    assert_eq!(run(&root, &["extra".to_string()]), ExitCode::FAILURE);
}
