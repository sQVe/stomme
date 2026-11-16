use super::Record;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RecordState {
    Matches,
    Differs,
    Missing,
}

pub(crate) fn record_state(record: Option<&Record>, path: &str, actual: &str) -> RecordState {
    match record.and_then(|record| record.files.get(path)) {
        Some(recorded) if recorded == actual => RecordState::Matches,
        Some(_) => RecordState::Differs,
        None => RecordState::Missing,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn record() -> Record {
        Record {
            version: "1".to_string(),
            files: BTreeMap::from([("docs/stomme/one.md".to_string(), "hash".to_string())]),
        }
    }

    #[test]
    fn record_state_compares_the_path_with_its_entry() {
        let record = record();

        assert_eq!(
            record_state(Some(&record), "docs/stomme/one.md", "hash"),
            RecordState::Matches
        );
        assert_eq!(
            record_state(Some(&record), "docs/stomme/one.md", "other"),
            RecordState::Differs
        );
        assert_eq!(
            record_state(Some(&record), "docs/stomme/two.md", "hash"),
            RecordState::Missing
        );
        assert_eq!(
            record_state(None, "docs/stomme/one.md", "hash"),
            RecordState::Missing
        );
    }
}
