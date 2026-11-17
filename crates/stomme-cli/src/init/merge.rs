use std::collections::BTreeMap;

use serde_json::Value as Json;
use toml::Value as Toml;

use super::InitError;

pub(crate) fn merge(target: &str, fragments: &[(&str, &str)]) -> Result<String, InitError> {
    if target.ends_with(".json") {
        let merged: Json = merge_documents(target, fragments, |text| {
            serde_json::from_str(text).map_err(|error| error.to_string())
        })?;
        let text =
            serde_json::to_string_pretty(&sorted(merged)).expect("a JSON value always serializes");

        return Ok(text + "\n");
    }

    if target.ends_with(".toml") {
        let merged: Toml = merge_documents(target, fragments, |text| {
            text.parse::<toml::Table>()
                .map(Toml::Table)
                .map_err(|error| error.message().to_string())
        })?;

        return Ok(toml::to_string(&merged).expect("a table parsed from TOML serializes"));
    }

    Err(InitError::MergeTarget {
        path: target.to_string(),
    })
}

fn merge_documents<D: Document + Clone>(
    target: &str,
    fragments: &[(&str, &str)],
    parse: impl Fn(&str) -> Result<D, String>,
) -> Result<D, InitError> {
    let mut parsed = Vec::new();

    for &(owner, text) in fragments {
        let document = parse(text).map_err(|reason| InitError::InvalidFragment {
            path: target.to_string(),
            owner: owner.to_string(),
            reason,
        })?;

        parsed.push((owner, document));
    }

    let ((_, first), rest) = parsed
        .split_first()
        .expect("the plan gives every merge target at least one fragment");
    let mut merged = first.clone();

    for (index, (owner, document)) in rest.iter().enumerate() {
        if let Err(path) = merged.absorb(document.clone(), &[]) {
            let (first, _) = parsed[..=index]
                .iter()
                .find(|(_, earlier)| lookup(earlier, &path).is_some())
                .expect("a clashing value comes from an earlier fragment");

            return Err(InitError::MergeClash {
                path: dotted(target, &path),
                first: (*first).to_string(),
                second: (*owner).to_string(),
            });
        }
    }

    Ok(merged)
}

fn lookup<'a, D: Document>(document: &'a D, path: &[String]) -> Option<&'a D> {
    path.iter().try_fold(document, |node, key| node.child(key))
}

fn dotted(target: &str, path: &[String]) -> String {
    if path.is_empty() {
        target.to_string()
    } else {
        path.join(".")
    }
}

trait Document: Sized + PartialEq {
    fn child(&self, key: &str) -> Option<&Self>;

    fn absorb(&mut self, incoming: Self, path: &[String]) -> Result<(), Vec<String>>;
}

impl Document for Json {
    fn child(&self, key: &str) -> Option<&Self> {
        self.get(key)
    }

    fn absorb(&mut self, incoming: Self, path: &[String]) -> Result<(), Vec<String>> {
        match (self, incoming) {
            (Self::Object(base), Self::Object(incoming)) => {
                for (key, value) in incoming {
                    match base.get_mut(&key) {
                        Some(existing) => existing.absorb(value, &child_path(path, &key))?,
                        None => {
                            base.insert(key, value);
                        }
                    }
                }

                Ok(())
            }
            (Self::Array(base), Self::Array(incoming)) => {
                union(base, incoming);
                Ok(())
            }
            (base, incoming) if *base == incoming => Ok(()),
            _ => Err(path.to_vec()),
        }
    }
}

impl Document for Toml {
    fn child(&self, key: &str) -> Option<&Self> {
        self.get(key)
    }

    fn absorb(&mut self, incoming: Self, path: &[String]) -> Result<(), Vec<String>> {
        match (self, incoming) {
            (Self::Table(base), Self::Table(incoming)) => {
                for (key, value) in incoming {
                    match base.get_mut(&key) {
                        Some(existing) => existing.absorb(value, &child_path(path, &key))?,
                        None => {
                            base.insert(key, value);
                        }
                    }
                }

                Ok(())
            }
            (Self::Array(base), Self::Array(incoming)) => {
                union(base, incoming);
                Ok(())
            }
            (base, incoming) if *base == incoming => Ok(()),
            _ => Err(path.to_vec()),
        }
    }
}

fn child_path(path: &[String], key: &str) -> Vec<String> {
    let mut child = path.to_vec();

    child.push(key.to_string());

    child
}

fn union<T: PartialEq>(base: &mut Vec<T>, incoming: Vec<T>) {
    for item in incoming {
        if !base.contains(&item) {
            base.push(item);
        }
    }
}

fn sorted(value: Json) -> Json {
    match value {
        Json::Object(object) => {
            let entries: BTreeMap<String, Json> = object
                .into_iter()
                .map(|(key, value)| (key, sorted(value)))
                .collect();

            Json::Object(entries.into_iter().collect())
        }
        Json::Array(items) => Json::Array(items.into_iter().map(sorted).collect()),
        other => other,
    }
}
