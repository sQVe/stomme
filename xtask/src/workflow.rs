const TOKEN_ACTION: &str = "actions/create-github-app-token";
const CONTENTS: &str = "permission-contents";

fn permission_problems(workflow: &str) -> Vec<String> {
    let mut token_lines = Vec::new();
    let mut read_contents_lines = Vec::new();
    let mut problems = Vec::new();

    for (index, line) in workflow.lines().enumerate() {
        let number = index + 1;

        if line.contains(TOKEN_ACTION) {
            token_lines.push(number);
        }

        let Some((key, value)) = permission_entry(line) else {
            continue;
        };

        if key == CONTENTS && value == "read" {
            read_contents_lines.push(number);
        }

        if value != "read" {
            problems.push(format!("line {number}: {key} is {value}"));
        }
    }

    let token_steps = token_lines.len();

    if token_steps == 0 {
        problems.push("line 1: no app token step".to_string());
    } else if read_contents_lines.len() != token_steps {
        problems.push(format!(
            "line {}: found {} permission-contents: read lines for {} token steps",
            token_lines[0],
            read_contents_lines.len(),
            token_steps
        ));
    }

    problems
}

fn permission_entry(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim();
    let entry = trimmed.strip_prefix("- ").unwrap_or(trimmed).trim();

    if entry.starts_with('#') {
        return None;
    }

    let (key, value) = entry.split_once(':')?;
    let key = strip_quotes(key.trim());
    let value = strip_quotes(strip_comment(value.trim()));

    if key.starts_with("permission-") {
        Some((key, value))
    } else {
        None
    }
}

fn strip_comment(value: &str) -> &str {
    match value.find(" #") {
        Some(index) => value[..index].trim(),
        None => value,
    }
}

fn strip_quotes(text: &str) -> &str {
    let unquoted = text
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
        .or_else(|| {
            text.strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
        });

    unquoted.unwrap_or(text)
}

#[cfg(test)]
mod tests;
