mod walk;

use std::cell::RefCell;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

use super::scopes::{self, ExportVisibility, Scope, Target};
use super::use_paths::{BindingTable, Namespace};
use super::{Module, is_crate_root, is_under_bin, keeps_exports, module_path, owner_module};
use crate::check::source_files::{SourceFile, Tree};
use walk::{MAX_DEPTH, Walk};

#[derive(Debug, thiserror::Error)]
#[error("a module path needs more than {limit} nested lookups")]
pub(super) struct DepthLimit {
    pub(super) limit: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Place {
    path: Vec<String>,
    folder: String,
    file: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Meaning {
    Module(Place),
    // A name bound inside a file the walk reached, which names no module.
    Item,
    // A name whose target lies outside the tree, so it may still name a module.
    Unresolved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Visibility {
    All,
    Crate,
}

type ScopeTable = BTreeMap<String, BTreeMap<Vec<String>, Scope>>;
type MemoKey = (Place, String, Visibility, Namespace);
type Memo = BTreeMap<MemoKey, BTreeSet<Meaning>>;

pub(super) struct Resolver<'a> {
    pub(super) tree: &'a Tree,
    pub(super) modules: &'a [Module],
    scopes: ScopeTable,
    file_paths: BTreeSet<&'a str>,
    memo: RefCell<Memo>,
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
            memo: RefCell::new(BTreeMap::new()),
        }
    }

    pub(super) fn reached_files(
        &self,
        target: &Module,
        caller_path: &[String],
        resolved: &[String],
        namespace: Namespace,
    ) -> Result<BTreeSet<String>, DepthLimit> {
        let visibility = if caller_path.starts_with(&target.path) {
            Visibility::All
        } else {
            Visibility::Crate
        };
        let segments = resolved.get(target.path.len()..).unwrap_or_default();
        let mut reached = BTreeSet::new();
        let Some((last, middle)) = segments.split_last() else {
            return Ok(reached);
        };
        let mut walk = Walk::new(target);
        let mut places = BTreeSet::from([entry_place(target)]);

        for segment in middle {
            let mut next = BTreeSet::new();

            for place in &places {
                let found = self.module_places(place, segment, visibility, &mut walk);

                if found.is_empty() {
                    reached.extend(place.file.clone());
                }

                next.extend(found);
            }

            places = next;
        }

        for place in &places {
            let meanings = self.lookup(place, last, visibility, namespace, &mut walk);
            reached.extend(terminal_files(place, &meanings));
        }

        // A deeper chain hides which file a use reaches, and no real module path is that deep,
        // so the run fails. The budget only caps work on a readable graph, so it reports nothing.
        if walk.depth_exceeded {
            return Err(DepthLimit { limit: MAX_DEPTH });
        }

        if walk.exhausted {
            return Ok(BTreeSet::new());
        }

        Ok(reached)
    }

    fn lookup(
        &self,
        place: &Place,
        name: &str,
        visibility: Visibility,
        namespace: Namespace,
        walk: &mut Walk,
    ) -> BTreeSet<Meaning> {
        if !walk.spend() {
            return BTreeSet::new();
        }

        let module = walk.module;

        if !place.path.starts_with(&module.path) {
            return ancestor_step(module, place, name);
        }

        let scope = self.scope(module, &place.path);
        let names_value = namespace == Namespace::Value
            && scope.is_some_and(|scope| scope.value_items.contains(name));

        if names_value {
            return BTreeSet::from([Meaning::Item]);
        }

        let children = self.child_meanings(module, place, name);
        let Some(scope) = scope else {
            return children;
        };
        let pair = (place.path.clone(), name.to_string());

        if walk.cut_if_open(&pair) {
            return children;
        }

        // A memo hit must not answer a pair that is already open; the cached value came from a
        // different context.
        let memo_key = (place.clone(), name.to_string(), visibility, namespace);
        let remembered = self.memo.borrow().get(&memo_key).cloned();

        if let Some(meanings) = remembered {
            return meanings;
        }

        if !walk.can_look_up() {
            return BTreeSet::new();
        }

        let open = walk.enter(pair);
        let (mut meanings, shadows_glob) =
            self.export_meanings(place, scope, name, visibility, walk);

        // A value and a module share a name in separate namespaces, so only an export that may
        // name the module hides the module a glob binds.
        if children.is_empty() && !shadows_glob {
            meanings.extend(self.glob_meanings(place, scope, name, visibility, namespace, walk));
        }

        let child_module_returned = !children.is_empty();
        meanings.extend(children);
        let cacheable = walk.leave(open, child_module_returned);

        if cacheable && !walk.exhausted {
            self.memo.borrow_mut().insert(memo_key, meanings.clone());
        }

        meanings
    }

    fn binds_publicly(&self, module: &Module, place: &Place, name: &str) -> bool {
        if !place.path.starts_with(&module.path) {
            return may_name_module(&ancestor_step(module, place, name));
        }

        let Some(scope) = self.scope(module, &place.path) else {
            return false;
        };
        let exported = scope.keeps_exports
            && scope.exports.iter().any(|export| {
                export.name == name && export.visibility != ExportVisibility::Restricted
            });
        let glob_binds_publicly = scope
            .globs
            .iter()
            .any(|glob| glob.visibility != ExportVisibility::Restricted);

        exported || glob_binds_publicly || scope.public_modules.contains(name)
    }

    fn export_meanings(
        &self,
        place: &Place,
        scope: &Scope,
        name: &str,
        visibility: Visibility,
        walk: &mut Walk,
    ) -> (BTreeSet<Meaning>, bool) {
        let mut meanings = BTreeSet::new();
        let mut shadows_glob = false;

        for export in scope.exports.iter().filter(|export| export.name == name) {
            let visible = match visibility {
                Visibility::All => true,
                Visibility::Crate => export.visibility != ExportVisibility::Restricted,
            };

            if !visible {
                continue;
            }

            let found = self.resolve_target(place, &export.target, walk);
            shadows_glob |= may_name_module(&found);

            if scope.keeps_exports {
                meanings.extend(found);
            }
        }

        (meanings, shadows_glob)
    }

    fn glob_meanings(
        &self,
        place: &Place,
        scope: &Scope,
        name: &str,
        visibility: Visibility,
        namespace: Namespace,
        walk: &mut Walk,
    ) -> BTreeSet<Meaning> {
        let mut meanings = BTreeSet::new();

        for glob in &scope.globs {
            let visible = match visibility {
                Visibility::All => true,
                Visibility::Crate => glob.visibility != ExportVisibility::Restricted,
            };

            if !visible {
                continue;
            }

            for meaning in self.resolve_target(place, &glob.target, walk) {
                let Meaning::Module(target) = meaning else {
                    continue;
                };

                if self.binds_publicly(walk.module, &target, name) {
                    meanings.extend(self.lookup(&target, name, visibility, namespace, walk));
                }
            }
        }

        meanings
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
            return BTreeSet::from([Meaning::Unresolved]);
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

    pub(super) fn declares_value_item(&self, module: &Module, path: &[String], name: &str) -> bool {
        self.scope(module, path)
            .is_some_and(|scope| scope.value_items.contains(name))
    }

    pub(super) fn bindings(&self, module: &Module, file_path: &[String]) -> BindingTable {
        let mut table = BindingTable::new();
        let Some(scopes) = self.scopes.get(&module.src_folder) else {
            return table;
        };

        for (scope_path, scope) in scopes {
            let Some(inline_path) = scope_path.strip_prefix(file_path) else {
                continue;
            };

            if inline_path != scope.inline_path {
                continue;
            }

            let bindings = table.entry(scope.inline_path.clone()).or_default();

            for binding in &scope.bindings {
                bindings.insert(binding.name.clone(), binding.clone());
            }
        }

        table
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
        place: &Place,
        segment: &str,
        visibility: Visibility,
        walk: &mut Walk,
    ) -> BTreeSet<Place> {
        let meanings = self.lookup(place, segment, visibility, Namespace::Type, walk);

        meanings
            .into_iter()
            .filter_map(|meaning| match meaning {
                Meaning::Module(found) => Some(found),
                Meaning::Item | Meaning::Unresolved => None,
            })
            .collect()
    }

    fn resolve_target(&self, place: &Place, target: &Target, walk: &mut Walk) -> BTreeSet<Meaning> {
        match target {
            Target::External => BTreeSet::from([Meaning::Unresolved]),
            Target::Bare(segments) => self.walk_target(place, segments, walk),
            Target::Relative {
                supers: 0,
                segments,
            } => self.walk_target(place, segments, walk),
            Target::Relative { supers, segments } => {
                let Some(kept) = place.path.len().checked_sub(*supers) else {
                    return BTreeSet::from([Meaning::Unresolved]);
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
            return BTreeSet::from([Meaning::Unresolved]);
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
                let found = self.module_places(place, segment, Visibility::All, walk);

                if found.is_empty() {
                    meanings.insert(Meaning::Unresolved);
                }

                next.extend(found);
            }

            places = next;
        }

        for place in &places {
            let found = self.lookup(place, last, Visibility::All, Namespace::Type, walk);

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
    let module_depth = if is_crate_root(&file.path, &module.src_folder) {
        0
    } else {
        file_path.len()
    };

    for mut scope in scopes::scopes(source, module.edition, module_depth) {
        // A glob in a private file still binds names inside it, and the walk needs those names
        // to reach the module a use names. Its explicit exports only hide its own globs.
        scope.keeps_exports = keep_exports;

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
                stored.value_items.append(&mut scope.value_items);
                stored.bindings.append(&mut scope.bindings);
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

fn may_name_module(meanings: &BTreeSet<Meaning>) -> bool {
    meanings
        .iter()
        .any(|meaning| !matches!(meaning, Meaning::Item))
}

fn terminal_files(place: &Place, meanings: &BTreeSet<Meaning>) -> Vec<String> {
    let names_item = meanings.is_empty()
        || meanings
            .iter()
            .any(|meaning| !matches!(meaning, Meaning::Module(_)));

    if names_item {
        return place.file.iter().cloned().collect();
    }

    meanings
        .iter()
        .filter_map(|meaning| match meaning {
            Meaning::Module(found) => found.file.clone(),
            Meaning::Item | Meaning::Unresolved => None,
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
