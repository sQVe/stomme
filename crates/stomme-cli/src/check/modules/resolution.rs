mod walk;

use std::cell::RefCell;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use super::scopes::{self, Scope, Target};
use super::{Module, is_crate_root, is_under_bin, keeps_exports, module_path, owner_module};
use crate::check::source_files::{SourceFile, Tree};
use walk::Walk;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Visibility {
    All,
    Public,
}

type ScopeTable = BTreeMap<String, BTreeMap<Vec<String>, Scope>>;

pub(super) struct Resolver<'a> {
    pub(super) tree: &'a Tree,
    pub(super) modules: &'a [Module],
    scopes: ScopeTable,
    file_paths: BTreeSet<&'a str>,
    glob_targets: RefCell<BTreeMap<Place, Rc<BTreeSet<Place>>>>,
    memo: RefCell<BTreeMap<(Place, String, Visibility), BTreeSet<Meaning>>>,
}

impl<'a> Resolver<'a> {
    pub(super) fn new(
        tree: &'a Tree,
        modules: &'a [Module],
        sources: &[(&SourceFile, String)],
    ) -> Self {
        let mut scopes = ScopeTable::new();

        for (file, source) in sources {
            let Some(module) = owner_module(modules, &file.path) else {
                continue;
            };
            let keep_exports = keeps_exports(module, &file.path);
            let skipped = is_crate_root(&file.path, &module.src_folder)
                || is_under_bin(&file.path, &module.src_folder);

            if skipped && !keep_exports {
                continue;
            }

            read_scopes(module, file, source, keep_exports, &mut scopes);
        }

        let file_paths = tree.files.iter().map(|file| file.path.as_str()).collect();

        Self {
            tree,
            modules,
            scopes,
            file_paths,
            glob_targets: RefCell::new(BTreeMap::new()),
            memo: RefCell::new(BTreeMap::new()),
        }
    }

    pub(super) fn reached_files(&self, module: &Module, resolved: &[String]) -> BTreeSet<String> {
        let segments = resolved.get(module.path.len()..).unwrap_or_default();
        let mut reached = BTreeSet::new();
        let Some((last, middle)) = segments.split_last() else {
            return reached;
        };
        let mut walk = Walk::new(module);
        let mut places = BTreeSet::from([entry_place(module)]);

        for segment in middle {
            let mut next = BTreeSet::new();

            for place in &places {
                let found = self.module_places(place, segment, &mut walk);

                if found.is_empty() {
                    reached.extend(place.file.clone());
                }

                next.extend(found);
            }

            places = next;
        }

        for place in &places {
            let meanings = self.lookup(place, last, Visibility::All, &mut walk);
            reached.extend(terminal_files(place, &meanings));
        }

        if walk.exhausted {
            return BTreeSet::new();
        }

        reached
    }

    fn lookup(
        &self,
        place: &Place,
        name: &str,
        visibility: Visibility,
        walk: &mut Walk,
    ) -> BTreeSet<Meaning> {
        if !walk.spend() {
            return BTreeSet::new();
        }

        let module = walk.module;

        if !place.path.starts_with(&module.path) {
            return match visibility {
                Visibility::All => ancestor_step(module, place, name),
                Visibility::Public => BTreeSet::new(),
            };
        }

        let memo_key = (place.clone(), name.to_string(), visibility);
        let remembered = self.memo.borrow().get(&memo_key).cloned();

        if let Some(meanings) = remembered {
            return meanings;
        }

        let children = self.child_meanings(module, place, name);
        let Some(scope) = self.scope(module, &place.path) else {
            return match visibility {
                Visibility::All => children,
                Visibility::Public => BTreeSet::new(),
            };
        };
        let pair = (place.path.clone(), name.to_string());

        if walk.cut_if_open(&pair) {
            return visible_children(scope, name, children, visibility);
        }

        let open = walk.enter(pair);
        let (mut meanings, exported) = self.export_meanings(place, scope, name, walk);

        if children.is_empty() && !exported {
            meanings.extend(self.glob_meanings(place, scope, name, walk));
        }

        meanings.extend(visible_children(scope, name, children, visibility));
        let cacheable = walk.leave(open);

        if cacheable && !walk.exhausted {
            self.memo.borrow_mut().insert(memo_key, meanings.clone());
        }

        meanings
    }

    fn binds_publicly(&self, module: &Module, place: &Place, name: &str) -> bool {
        let Some(scope) = self.scope(module, &place.path) else {
            return false;
        };
        let exported = scope.exports.iter().any(|export| export.name == name);

        exported || !scope.globs.is_empty() || scope.public_modules.contains(name)
    }

    fn export_meanings(
        &self,
        place: &Place,
        scope: &Scope,
        name: &str,
        walk: &mut Walk,
    ) -> (BTreeSet<Meaning>, bool) {
        let mut meanings = BTreeSet::new();
        let mut exported = false;

        for export in scope.exports.iter().filter(|export| export.name == name) {
            exported = true;
            meanings.extend(self.resolve_target(place, &export.target, walk));
        }

        (meanings, exported)
    }

    fn glob_meanings(
        &self,
        place: &Place,
        scope: &Scope,
        name: &str,
        walk: &mut Walk,
    ) -> BTreeSet<Meaning> {
        let mut meanings = BTreeSet::new();

        for target in self.glob_targets(place, scope, walk).iter() {
            if !self.binds_publicly(walk.module, target, name) {
                continue;
            }

            meanings.extend(self.lookup(target, name, Visibility::Public, walk));
        }

        meanings
    }

    fn glob_targets(&self, place: &Place, scope: &Scope, walk: &mut Walk) -> Rc<BTreeSet<Place>> {
        let remembered = self.glob_targets.borrow().get(place).cloned();

        if let Some(targets) = remembered {
            return targets;
        }

        let outer_cut = walk.begin_nested();
        let mut targets = BTreeSet::new();

        for glob in &scope.globs {
            for meaning in self.resolve_target(place, glob, walk) {
                if let Meaning::Module(target) = meaning {
                    targets.insert(target);
                }
            }
        }

        let cacheable = walk.end_nested(outer_cut);
        let targets = Rc::new(targets);

        if cacheable && !walk.exhausted {
            self.glob_targets
                .borrow_mut()
                .insert(place.clone(), Rc::clone(&targets));
        }

        targets
    }

    fn child_meanings(&self, module: &Module, place: &Place, name: &str) -> BTreeSet<Meaning> {
        let mut path = place.path.clone();
        path.push(name.to_string());
        let folder = format!("{}/{name}", place.folder);
        let file = format!("{folder}.rs");
        let has_file = self.file_paths.contains(file.as_str());
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
        self.scopes.get(&module.src_folder)?.get(path)
    }

    fn in_deeper_row(&self, module: &Module, path: &[String]) -> bool {
        self.modules.iter().any(|other| {
            other.src_folder == module.src_folder
                && other.path.len() > module.path.len()
                && path.starts_with(&other.path)
        })
    }

    fn module_places(&self, place: &Place, segment: &str, walk: &mut Walk) -> BTreeSet<Place> {
        let meanings = self.lookup(place, segment, Visibility::All, walk);

        meanings
            .into_iter()
            .filter_map(|meaning| match meaning {
                Meaning::Module(found) => Some(found),
                Meaning::Item => None,
            })
            .collect()
    }

    fn resolve_target(&self, place: &Place, target: &Target, walk: &mut Walk) -> BTreeSet<Meaning> {
        let item = BTreeSet::from([Meaning::Item]);

        match target {
            Target::External => item,
            Target::Bare(segments) => self.walk_target(place, segments, walk),
            Target::Relative {
                supers: 0,
                segments,
            } => self.walk_target(place, segments, walk),
            Target::Relative { supers, segments } => {
                let Some(kept) = place.path.len().checked_sub(*supers) else {
                    return item;
                };
                let mut absolute = place.path[..kept].to_vec();
                absolute.extend(segments.iter().cloned());

                self.resolve_absolute(&absolute, walk)
            }
            Target::Crate(segments) => self.resolve_absolute(segments, walk),
        }
    }

    fn resolve_absolute(&self, absolute: &[String], walk: &mut Walk) -> BTreeSet<Meaning> {
        let module = walk.module;

        if is_strict_ancestor(module, absolute) {
            return BTreeSet::from([Meaning::Module(ancestor_place(module, absolute))]);
        }

        let inside_row =
            absolute.starts_with(&module.path) && !self.in_deeper_row(module, absolute);

        if !inside_row {
            return BTreeSet::from([Meaning::Item]);
        }

        let start = entry_place(module);
        let segments = &absolute[module.path.len()..];

        self.walk_target(&start, segments, walk)
    }

    fn walk_target(
        &self,
        start: &Place,
        segments: &[String],
        walk: &mut Walk,
    ) -> BTreeSet<Meaning> {
        let Some((last, middle)) = segments.split_last() else {
            return BTreeSet::from([Meaning::Module(start.clone())]);
        };
        let mut meanings = BTreeSet::new();
        let mut places = BTreeSet::from([start.clone()]);

        for segment in middle {
            let mut next = BTreeSet::new();

            for place in &places {
                let found = self.module_places(place, segment, walk);

                if found.is_empty() {
                    meanings.insert(Meaning::Item);
                }

                next.extend(found);
            }

            places = next;
        }

        for place in &places {
            let found = self.lookup(place, last, Visibility::All, walk);

            if found.is_empty() {
                meanings.insert(Meaning::Item);
            }

            meanings.extend(found);
        }

        meanings
    }
}

fn read_scopes(
    module: &Module,
    file: &SourceFile,
    source: &str,
    keep_exports: bool,
    table: &mut ScopeTable,
) {
    let file_path = module_path(&file.path, &module.src_folder);

    for mut scope in scopes::scopes(source) {
        if !keep_exports {
            scope.exports.clear();
            scope.globs.clear();
        }

        let mut scope_path = file_path.clone();
        scope_path.extend(scope.inline_path.iter().cloned());
        let folder_scopes = table.entry(module.src_folder.clone()).or_default();

        match folder_scopes.entry(scope_path) {
            Entry::Vacant(vacant) => {
                vacant.insert(scope);
            }
            Entry::Occupied(mut occupied) => {
                let stored = occupied.get_mut();
                stored.exports.append(&mut scope.exports);
                stored.globs.append(&mut scope.globs);
                stored.inline_modules.append(&mut scope.inline_modules);
                stored.public_modules.append(&mut scope.public_modules);
            }
        }
    }
}

fn entry_place(module: &Module) -> Place {
    Place {
        path: module.path.clone(),
        folder: module.owner_folder.clone(),
        file: None,
    }
}

fn ancestor_step(module: &Module, place: &Place, name: &str) -> BTreeSet<Meaning> {
    let mut path = place.path.clone();
    path.push(name.to_string());

    if path == module.path {
        return BTreeSet::from([Meaning::Module(entry_place(module))]);
    }

    if is_strict_ancestor(module, &path) {
        return BTreeSet::from([Meaning::Module(ancestor_place(module, &path))]);
    }

    BTreeSet::from([Meaning::Item])
}

fn is_strict_ancestor(module: &Module, path: &[String]) -> bool {
    path.len() < module.path.len() && module.path.starts_with(path)
}

fn visible_children(
    scope: &Scope,
    name: &str,
    children: BTreeSet<Meaning>,
    visibility: Visibility,
) -> BTreeSet<Meaning> {
    let hidden = visibility == Visibility::Public && !scope.public_modules.contains(name);

    if hidden {
        return BTreeSet::new();
    }

    children
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

fn ancestor_place(module: &Module, path: &[String]) -> Place {
    let folder = if path.is_empty() {
        module.src_folder.clone()
    } else {
        format!("{}/{}", module.src_folder, path.join("/"))
    };

    Place {
        path: path.to_vec(),
        folder,
        file: None,
    }
}

#[cfg(test)]
mod tests;
