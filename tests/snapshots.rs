use {
    miette::Diagnostic,
    std::{fmt, fs, path::Path},
    thiserror::Error,
    tracing_filter::simple,
};

#[derive(Debug, Error)]
#[error(transparent)]
struct StripDiagnosticUrl<'a>(&'a dyn Diagnostic);

impl Diagnostic for StripDiagnosticUrl<'_> {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.0.code()
    }

    fn severity(&self) -> Option<miette::Severity> {
        self.0.severity()
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.0.help()
    }

    fn url<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        None
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        self.0.source_code()
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        self.0.labels()
    }

    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn miette::Diagnostic> + 'a>> {
        self.0.related().map(
            |iter| -> Box<dyn Iterator<Item = &'a dyn miette::Diagnostic> + 'a> {
                Box::new(
                    // leakage is sad but required to strip URLs from the test output [zkat/miette#136]
                    iter.map(|d| -> &dyn Diagnostic { Box::leak(Box::new(StripDiagnosticUrl(d))) }),
                )
            },
        )
    }
}

struct DisplayDiagnostic<'a>(&'a dyn Diagnostic);

impl fmt::Display for DisplayDiagnostic<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        miette::GraphicalReportHandler::new_themed(miette::GraphicalTheme::unicode_nocolor())
            .with_links(false)
            .render_report(f, &StripDiagnosticUrl(self.0))
            .or_else(|_| write!(f, "failed to pretty-print {:#?}", self.0))
    }
}

#[test]
#[cfg(feature = "regex")]
fn snapshot_simple_filter_parser() {
    fn callback(path: &Path) {
        let src = fs::read_to_string(path).unwrap();
        let (filter, report) = simple::Filter::parse(&src);

        if let Some(filter) = filter {
            insta::assert_snapshot!(Some("simple_filter"), format!("{filter:#?}"), &src);
        } else {
            insta::assert_snapshot!(Some("simple_filter"), "(compilation failed)", &src);
        }
        if let Some(report) = report {
            let report = DisplayDiagnostic(&*report);
            insta::assert_snapshot!(Some("simple_report"), format!("{report}"), &src);
        } else {
            insta::assert_snapshot!(Some("simple_report"), "(no warnings)", &src);
        }
    }

    insta::glob!("test_cases/*.env_filter", callback);
}
