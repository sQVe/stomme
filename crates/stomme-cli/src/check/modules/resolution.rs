use std::collections::{BTreeMap, BTreeSet};

use super::scopes::{self, Scope, Target};
use super::{Module, module_path};
use crate::check::source_files::Tree;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Place {
    path: Vec<String>,
    folder: String,
    file: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Meaning {
    Module(Place),
    Item,
}

type ScopeKey = (String, Vec<String>);
type InProgress = BTreeSet<(Vec<String>, String)>;

pub(super) struct Resolver<'a> {
    pub(super) tree: &'a Tree,
    pub(super) modules: &'a [Module],
    scopes: BTreeMap<ScopeKey, Scope>,
}

impl<'a> Resolver<'a> {
    pub(super) fn new(tree: &'a Tree, modules: &'a [Module]) -> anyhow::Result<Self> {
        let mut scopes = BTreeMap::new();
        let mut read = BTreeSet::new();

        for module in modules {
            let entry = std::iter::once(&module.entry);

            for path in entry.chain(&module.public) {
                if read.insert(path) {
                    read_scopes(tree, module, path, &mut scopes)?;
                }
            }
        }

        Ok(Self {
            tree,
            modules,
            scopes,
        })
    }

    pub(super) fn reached_files(&self, module: &Module, resolved: &[String]) -> BTreeSet<String> {
        let segments = resolved.get(module.path.len()..).unwrap_or_default();
        let mut reached = BTreeSet::new();
        let Some((last, middle)) = segments.split_last() else {
            return reached;
        };
        let mut in_progress = InProgress::new();
        let mut places = BTreeSet::from([entry_place(module)]);

        for segment in middle {
            let mut next = BTreeSet::new();

            for place in &places {
                let found = self.module_places(module, place, segment, &mut in_progress);

                if found.is_empty() {
                    reached.extend(place.file.clone());
                }

                next.extend(found);
            }

            places = next;
        }

        for place in &places {
            let meanings = self.lookup(module, place, last, &mut in_progress);
            reached.extend(terminal_files(place, &meanings));
        }

        reached
    }

    fn lookup(
        &self,
        module: &Module,
        place: &Place,
        name: &str,
        in_progress: &mut InProgress,
    ) -> BTreeSet<Meaning> {
        let mut meanings = self.child_meanings(module, place, name);
        let Some(scope) = self.scope(module, &place.path) else {
            return meanings;
        };
        let pair = (place.path.clone(), name.to_string());

        if !in_progress.insert(pair.clone()) {
            return meanings;
        }

        for export in scope.exports.iter().filter(|export| export.name == name) {
            meanings.extend(self.resolve_target(module, place, &export.target, in_progress));
        }

        in_progress.remove(&pair);

        meanings
    }

    fn child_meanings(&self, module: &Module, place: &Place, name: &str) -> BTreeSet<Meaning> {
        let mut path = place.path.clone();
        path.push(name.to_string());
        let folder = format!("{}/{name}", place.folder);
        let file = format!("{folder}.rs");
        let has_file = self.tree.files.iter().any(|source| source.path == file);
        let has_folder = self.tree.folders.contains(&folder);
        let has_inline = self
            .scope(module, &place.path)
            .is_some_and(|scope| scope.inline_modules.contains(name));
        let child_file = if has_file {
            Some(file)
        } else if has_folder || has_inline {
            place.file.clone()
        } else {
            return BTreeSet::new();
        };

        if self.in_deeper_row(module, &path) {
            return BTreeSet::from([Meaning::Item]);
        }

        let child = Place {
            path,
            folder,
            file: child_file,
        };

        BTreeSet::from([Meaning::Module(child)])
    }

    fn scope(&self, module: &Module, path: &[String]) -> Option<&Scope> {
        self.scopes.get(&(module.src_folder.clone(), path.to_vec()))
    }

    fn in_deeper_row(&self, module: &Module, path: &[String]) -> bool {
        self.modules.iter().any(|other| {
            other.src_folder == module.src_folder
                && other.path.len() > module.path.len()
                && path.starts_with(&other.path)
        })
    }

    fn module_places(
        &self,
        module: &Module,
        place: &Place,
        segment: &str,
        in_progress: &mut InProgress,
    ) -> BTreeSet<Place> {
        let meanings = self.lookup(module, place, segment, in_progress);

        meanings
            .into_iter()
            .filter_map(|meaning| match meaning {
                Meaning::Module(found) => Some(found),
                Meaning::Item => None,
            })
            .collect()
    }

    fn resolve_target(
        &self,
        module: &Module,
        place: &Place,
        target: &Target,
        in_progress: &mut InProgress,
    ) -> BTreeSet<Meaning> {
        let item = BTreeSet::from([Meaning::Item]);

        match target {
            Target::External => item,
            Target::Bare(segments) => self.walk_target(module, place, segments, in_progress),
            Target::Relative {
                supers: 0,
                segments,
            } => self.walk_target(module, place, segments, in_progress),
            Target::Relative { supers, segments } => {
                let Some(kept) = place.path.len().checked_sub(*supers) else {
                    return item;
                };
                let mut absolute = place.path[..kept].to_vec();
                absolute.extend(segments.iter().cloned());

                self.resolve_absolute(module, &absolute, in_progress)
            }
            Target::Crate(segments) => self.resolve_absolute(module, segments, in_progress),
        }
    }

    fn resolve_absolute(
        &self,
        module: &Module,
        absolute: &[String],
        in_progress: &mut InProgress,
    ) -> BTreeSet<Meaning> {
        let inside_row =
            absolute.starts_with(&module.path) && !self.in_deeper_row(module, absolute);

        if !inside_row {
            return BTreeSet::from([Meaning::Item]);
        }

        let start = entry_place(module);
        let segments = &absolute[module.path.len()..];

        self.walk_target(module, &start, segments, in_progress)
    }

    fn walk_target(
        &self,
        module: &Module,
        start: &Place,
        segments: &[String],
        in_progress: &mut InProgress,
    ) -> BTreeSet<Meaning> {
        let Some((last, middle)) = segments.split_last() else {
            return BTreeSet::from([Meaning::Module(start.clone())]);
        };
        let mut meanings = BTreeSet::new();
        let mut places = BTreeSet::from([start.clone()]);

        for segment in middle {
            let mut next = BTreeSet::new();

            for place in &places {
                let found = self.module_places(module, place, segment, in_progress);

                if found.is_empty() {
                    meanings.insert(Meaning::Item);
                }

                next.extend(found);
            }

            places = next;
        }

        for place in &places {
            let found = self.lookup(module, place, last, in_progress);

            if found.is_empty() {
                meanings.insert(Meaning::Item);
            }

            meanings.extend(found);
        }

        meanings
    }
}

fn read_scopes(
    tree: &Tree,
    module: &Module,
    path: &str,
    table: &mut BTreeMap<ScopeKey, Scope>,
) -> anyhow::Result<()> {
    let Some(file) = tree.files.iter().find(|file| file.path == path) else {
        return Ok(());
    };
    let source = tree.read(file)?;
    let file_path = module_path(path, &module.src_folder);

    for scope in scopes::scopes(&source) {
        let mut scope_path = file_path.clone();
        scope_path.extend(scope.inline_path.iter().cloned());
        table.insert((module.src_folder.clone(), scope_path), scope);
    }

    Ok(())
}

fn entry_place(module: &Module) -> Place {
    Place {
        path: module.path.clone(),
        folder: module.owner_folder.clone(),
        file: None,
    }
}

fn terminal_files(place: &Place, meanings: &BTreeSet<Meaning>) -> Vec<String> {
    let names_item = meanings.is_empty() || meanings.contains(&Meaning::Item);

    if names_item {
        return place.file.iter().cloned().collect();
    }

    meanings
        .iter()
        .filter_map(|meaning| match meaning {
            Meaning::Module(found) => found.file.clone(),
            Meaning::Item => None,
        })
        .collect()
}

#[cfg(test)]
mod tests;
