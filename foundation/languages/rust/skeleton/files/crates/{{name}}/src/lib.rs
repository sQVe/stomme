pub fn greeting() -> &'static str {
    "Hello from {{name}}."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greeting_names_the_project() {
        assert_eq!(greeting(), "Hello from {{name}}.");
    }
}
