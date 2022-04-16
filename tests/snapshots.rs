#![cfg(not(target_family = "wasm"))]
#![cfg(feature = "fancy-errors")]

use {
    std::{fs, path::Path},
    tracing_filter::{legacy, simple},
};

fn decolor(s: String) -> String {
    String::from_utf8(strip_ansi_escapes::strip(s).unwrap()).unwrap()
}

#[test]
fn snapshot_simple_filter_parser() {
    fn callback(path: &Path) {
        let src = fs::read_to_string(path).unwrap();
        match simple::Filter::parse(&src) {
            (Some(filter), Some(report)) => {
                let report = format!("{report:?}");
                insta::assert_snapshot!(
                    Some("simple"),
                    format!("{filter}\n\n{}", decolor(report)),
                    &src
                )
            },
            (Some(filter), None) => {
                insta::assert_snapshot!(Some("simple"), format!("{filter}\n(no warnings)"), &src)
            },
            (None, Some(report)) => {
                let report = format!("{report:?}");
                insta::assert_snapshot!(
                    Some("simple"),
                    format!("(compilation failed)\n\n{}", decolor(report)),
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
                let report = format!("{report:?}");
                insta::assert_snapshot!(
                    Some("legacy"),
                    format!("{filter}\n\n{}", decolor(report)),
                    &src
                )
            },
            (filter, None) => {
                insta::assert_snapshot!(Some("legacy"), format!("{filter}\n\n(no warnings)"), &src)
            },
        }
    }

    insta::glob!("test_cases/*.env_filter", callback);
}
