use super::is_40_character_lowercase_hex_commit;

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

#[test]
fn accepts_only_a_40_character_lowercase_hex_commit() {
    let cases = [
        ("a valid commit", COMMIT.to_string(), true),
        ("39 characters", COMMIT[..39].to_string(), false),
        ("41 characters", format!("{COMMIT}0"), false),
        ("an uppercase character", COMMIT.to_uppercase(), false),
        ("a non-hex character", format!("{}g", &COMMIT[..39]), false),
    ];

    for (name, value, expected) in cases {
        assert_eq!(
            is_40_character_lowercase_hex_commit(&value),
            expected,
            "{name}: {value}"
        );
    }
}
