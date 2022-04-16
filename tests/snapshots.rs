#![cfg(not(target_family = "wasm"))]

use {
    std::{fs, path::Path},
    tracing_filter::{legacy, simple, DiagnosticsTheme},
};

#[test]
fn snapshot_simple_filter_parser() {
    fn callback(path: &Path) {
        let src = fs::read_to_string(path).unwrap();
        match simple::Filter::parse(&src) {
            (Some(filter), Some(report)) => {
                assert!(!report.is_error());
                let warn = report
                    .warn(DiagnosticsTheme::UnicodeNocolor)
                    .map(|x| x.to_string())
                    .unwrap_or_default();
                insta::assert_snapshot!(Some("simple"), format!("{filter}\n\n{warn}"), &src)
            },
            (Some(filter), None) => {
                insta::assert_snapshot!(Some("simple"), format!("{filter}\n(no warnings)"), &src)
            },
            (None, Some(report)) => {
                assert!(!report.is_warning());
                let error = report
                    .error(DiagnosticsTheme::UnicodeNocolor)
                    .map(|x| x.to_string())
                    .unwrap_or_default();
                insta::assert_snapshot!(
                    Some("simple"),
                    format!("(compilation failed)\n\n{error}"),
                    &src
                )
            },
            (None, None) => {
                insta::assert_snapshot!(Some("simple"), "(compilation failed)\n(no warnings)", &src)
            },
        }
    }

    insta::glob!("test_cases/*.env_filter", callback);
}

#[test]
fn snapshot_legacy_filter_parser() {
    fn callback(path: &Path) {
        let src = fs::read_to_string(path).unwrap();
        match legacy::Filter::parse(&src) {
            (filter, Some(report)) => {
                assert!(!report.is_error());
                let warn = report
                    .warn(DiagnosticsTheme::UnicodeNocolor)
                    .map(|x| x.to_string())
                    .unwrap_or_default();
                insta::assert_snapshot!(Some("legacy"), format!("{filter}\n\n{warn}"), &src)
            },
            (filter, None) => {
                insta::assert_snapshot!(Some("legacy"), format!("{filter}\n\n(no warnings)"), &src)
            },
        }
    }

    insta::glob!("test_cases/*.env_filter", callback);
}
