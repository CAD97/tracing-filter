use {
    miette::Diagnostic,
    std::{fmt, fs, path::Path},
    tracing_filter::{legacy, simple},
};

struct DisplayDiagnostic<'a>(&'a dyn Diagnostic);

impl fmt::Display for DisplayDiagnostic<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        miette::GraphicalReportHandler::new_themed(miette::GraphicalTheme::unicode_nocolor())
            .with_urls(false)
            .render_report(f, self.0)
            .or_else(|_| write!(f, "failed to pretty-print {:#?}", self.0))
    }
}

#[test]
fn snapshot_simple_filter_parser() {
    fn callback(path: &Path) {
        let src = fs::read_to_string(path).unwrap();
        match simple::Filter::parse(&src) {
            (Some(filter), Some(report)) => {
                insta::assert_snapshot!(
                    Some("simple"),
                    format!("{filter}\n{}", DisplayDiagnostic(&*report)),
                    &src
                )
            },
            (Some(filter), None) => {
                insta::assert_snapshot!(Some("simple"), format!("{filter}\n(no warnings)"), &src)
            },
            (None, Some(report)) => {
                insta::assert_snapshot!(
                    Some("simple"),
                    format!("(compilation failed)\n{}", DisplayDiagnostic(&*report)),
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
                insta::assert_snapshot!(
                    Some("legacy"),
                    format!("{filter}\n{}", DisplayDiagnostic(&*report)),
                    &src
                )
            },
            (filter, None) => {
                insta::assert_snapshot!(Some("legacy"), format!("{filter}\n(no warnings)"), &src)
            },
        }
    }

    insta::glob!("test_cases/*.env_filter", callback);
}
