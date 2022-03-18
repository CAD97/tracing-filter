use {
    std::{fmt, fs, path::Path},
    tracing_filter::simple::Filter,
};

struct DisplayDiagnostic<'a>(&'a dyn miette::Diagnostic);

impl fmt::Display for DisplayDiagnostic<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        miette::GraphicalReportHandler::new_themed(miette::GraphicalTheme::unicode_nocolor())
            .with_links(false)
            .render_report(f, self.0)
            .or_else(|_| write!(f, "failed to pretty-print {:#?}", self.0))
    }
}

#[test]
fn snapshot_filter_parser() {
    fn callback(path: &Path) {
        let src = fs::read_to_string(path).unwrap();
        let (filter, report) = Filter::parse(&src);

        if let Some(filter) = filter {
            insta::assert_snapshot!(Some("filter"), format!("{filter:#?}"), &src);
        } else {
            insta::assert_snapshot!(Some("filter"), "(compilation failed)", &src);
        }
        if let Some(report) = report {
            let report = DisplayDiagnostic(&*report);
            insta::assert_snapshot!(Some("report"), format!("{report}"), &src);
        } else {
            insta::assert_snapshot!(Some("report"), "(no warnings)", &src);
        }
    }

    insta::glob!("test_cases/*.env_filter", callback);
}
