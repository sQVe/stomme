#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Detection {
    /// `Cargo.lock` lists a package with this name, from any source.
    CargoPackage(&'static str),
    /// The root `package.json` lists this package in `dependencies`, `devDependencies`, or
    /// `peerDependencies`.
    PackageDependency(&'static str),
    /// The root `package.json` has this top-level field.
    PackageField(&'static str),
    /// A source folder holds both files, such as a Herdr mod's `mod.json` beside `main.ts`.
    FilesTogether {
        manifest: &'static str,
        entry: &'static str,
    },
}
