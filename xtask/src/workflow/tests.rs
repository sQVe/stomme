use super::*;

const TOKEN_ACTION: &str = "actions/create-github-app-token@64ab";
const RELEASE_GATE: &str = include_str!("../../../.github/workflows/release-gate.yml");

fn token_step(action: &str, permissions: &[&str]) -> String {
    let mut step = format!("      - uses: {action}\n        with:\n          app-id: app\n");

    for permission in permissions {
        step.push_str("          ");
        step.push_str(permission);
        step.push('\n');
    }

    step
}

#[test]
fn permission_problems_decides_from_permission_lines() {
    let cases: [(&str, String, bool); 10] = [
        (
            "contents read",
            token_step(TOKEN_ACTION, &["permission-contents: read"]),
            true,
        ),
        ("no permission input", token_step(TOKEN_ACTION, &[]), false),
        (
            "pull requests write",
            token_step(
                TOKEN_ACTION,
                &[
                    "permission-contents: read",
                    "permission-pull-requests: write",
                ],
            ),
            false,
        ),
        (
            "contents write",
            token_step(TOKEN_ACTION, &["permission-contents: write"]),
            false,
        ),
        (
            "quoted read",
            token_step(TOKEN_ACTION, &["permission-contents: 'read'"]),
            true,
        ),
        (
            "trailing comment",
            token_step(TOKEN_ACTION, &["permission-contents: read # why"]),
            true,
        ),
        (
            "quoted key",
            token_step(TOKEN_ACTION, &["\"permission-contents\": read"]),
            true,
        ),
        (
            "quoted action without permission",
            token_step("\"actions/create-github-app-token@64ab\"", &[]),
            false,
        ),
        (
            "two token steps with one permission",
            format!(
                "{}{}",
                token_step(TOKEN_ACTION, &["permission-contents: read"]),
                token_step(TOKEN_ACTION, &[])
            ),
            false,
        ),
        (
            "expression value",
            token_step(TOKEN_ACTION, &["permission-contents: ${{ x }}"]),
            false,
        ),
    ];

    for (name, workflow, accepted) in cases {
        let problems = permission_problems(&workflow);

        assert_eq!(problems.is_empty(), accepted, "{name}: {problems:?}");
    }
}

#[test]
fn release_gate_token_steps_request_read_only_contents() {
    let token_steps = RELEASE_GATE
        .lines()
        .filter(|line| line.contains("actions/create-github-app-token"))
        .count();

    assert!(token_steps > 0, "the release gate creates an app token");
    assert_eq!(permission_problems(RELEASE_GATE), Vec::<String>::new());
}
