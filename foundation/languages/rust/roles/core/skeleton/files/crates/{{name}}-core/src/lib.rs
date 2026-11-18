pub fn role() -> &'static str {
    "core"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_names_the_crate_role() {
        assert_eq!(role(), "core");
    }
}
