use std::collections::BTreeMap;
use std::sync::LazyLock;

const PACKAGES: &str = include_str!("../../../packages.toml");

static PINS: LazyLock<BTreeMap<String, String>> =
    LazyLock::new(|| toml::from_str(PACKAGES).expect("packages.toml is valid TOML"));

pub(crate) fn pins() -> &'static BTreeMap<String, String> {
    &PINS
}
