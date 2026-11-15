use super::*;
use crate::layer;
use crate::test_support::{complete_project, write_file};

#[test]
fn run_reports_a_conflicting_pair_in_either_order() {
    let catalog = layer::test_catalog(&[
        ("one", "language = \"rust\"\nconflicts = [\"two\"]\n"),
        ("two", "language = \"rust\"\n"),
    ]);

    for listed in ["\"one\", \"two\"", "\"two\", \"one\""] {
        let root = complete_project();

        write_file(
            root.path(),
            CONFIG_FILE,
            &format!("schema = 1\nlanguage = \"rust\"\nlayers = [{listed}]\n"),
        );

        let problems = run(root.path(), &catalog).expect("check the project");

        assert_eq!(problems.len(), 1, "{listed}: {problems:?}");
        assert_eq!(problems[0].path, CONFIG_FILE, "{listed}");
        assert_eq!(problems[0].rule, Rule::BrokenInstallation, "{listed}");
        for name in ["`one`", "`two`"] {
            assert!(
                problems[0].message.contains(name),
                "{listed}: {name}: {}",
                problems[0].message
            );
        }
    }
}
