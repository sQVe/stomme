#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Detection {
    CargoPackage(&'static str),
    PackageDependency(&'static str),
    PackageField(&'static str),
    FilesTogether {
        manifest: &'static str,
        entry: &'static str,
    },
}
