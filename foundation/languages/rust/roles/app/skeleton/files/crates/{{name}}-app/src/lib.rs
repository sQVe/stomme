pub fn role() -> &'static str {
    "app"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_names_the_crate_role() {
        assert_eq!(role(), "app");
    }
}
