use miette::{GraphicalReportHandler, GraphicalTheme};

use {
    miette::{Diagnostic, ReportHandler},
    std::{borrow::Cow, error::Error, fmt},
};

pub struct Diagnostics<'a> {
    pub(crate) error: Option<Box<dyn Diagnostic + Send + Sync + 'static>>,
    pub(crate) ignored: Vec<Box<dyn Diagnostic + Send + Sync + 'static>>,
    pub(crate) disabled: Option<Box<dyn Diagnostic + Send + Sync + 'static>>,
    pub(crate) source: Cow<'a, str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticsTheme {
    Ascii,
    AsciiNocolor,
    Unicode,
    UnicodeNocolor,
    Guess,
}

impl DiagnosticsTheme {
    fn report_handler(self) -> GraphicalReportHandler {
        match self {
            Self::Ascii => GraphicalReportHandler::new_themed(GraphicalTheme::ascii()),
            Self::AsciiNocolor => GraphicalReportHandler::new_themed(GraphicalTheme::none()),
            Self::Unicode => GraphicalReportHandler::new_themed(GraphicalTheme::unicode()),
            Self::UnicodeNocolor => {
                GraphicalReportHandler::new_themed(GraphicalTheme::unicode_nocolor())
            },
            Self::Guess => GraphicalReportHandler::new(),
        }
    }
}

impl Diagnostics<'_> {
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    pub fn is_warning(&self) -> bool {
        !self.ignored.is_empty() || self.disabled.is_some()
    }

    pub fn is_empty(&self) -> bool {
        !self.is_error() && !self.is_warning()
    }

    pub fn into_owned(self) -> Diagnostics<'static> {
        Diagnostics {
            error: self.error,
            ignored: self.ignored,
            disabled: self.disabled,
            source: Cow::Owned(self.source.into()),
        }
    }

    /// Any errors generated by parsing a filter directive string. This means
    /// that no filters were applied! You should probably `error!` this into
    /// your logging backend.
    pub fn error(&self, theme: DiagnosticsTheme) -> Option<impl fmt::Display + '_> {
        struct ErrorDiagnostics<'a>(&'a Diagnostics<'a>, DiagnosticsTheme);
        impl fmt::Display for ErrorDiagnostics<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let report_handler = self.1.report_handler();
                if let Some(error) = &self.0.error {
                    report_handler.debug(&DiagnosticWithStr::new(&**error, &self.0.source), f)?;
                }
                Ok(())
            }
        }
        if self.is_error() {
            Some(ErrorDiagnostics(self, theme))
        } else {
            None
        }
    }

    /// Any errors generated by parsing a filter directive string. This means
    /// that some filters were not applied! You should probably `warn!` this
    /// into your logging backend.
    pub fn warn(&self, theme: DiagnosticsTheme) -> Option<impl fmt::Display + '_> {
        struct WarnDiagnostics<'a>(&'a Diagnostics<'a>, DiagnosticsTheme);
        impl fmt::Display for WarnDiagnostics<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let report_handler = self.1.report_handler();

                if !self.0.ignored.is_empty() {
                    writeln!(
                        f,
                        "{} directives were ignored as invalid",
                        self.0.ignored.len()
                    )?;
                }

                for ignored in &self.0.ignored {
                    report_handler.debug(&DiagnosticWithStr::new(&**ignored, &self.0.source), f)?;
                }

                if let Some(disabled) = &self.0.disabled {
                    report_handler
                        .debug(&DiagnosticWithStr::new(&**disabled, &self.0.source), f)?;
                }

                Ok(())
            }
        }
        if self.is_warning() {
            Some(WarnDiagnostics(self, theme))
        } else {
            None
        }
    }
}

impl fmt::Display for Diagnostics<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            f.debug_struct(stringify!(Diagnostics))
                .field(stringify!(error), &self.error)
                .field(stringify!(ignored), &self.ignored)
                .field(stringify!(disabled), &self.disabled)
                .finish()
        } else {
            if let Some(error) = &self.error {
                writeln!(f, "{}", error)?;
            }

            if !self.ignored.is_empty() {
                writeln!(
                    f,
                    "{} directives were ignored as invalid",
                    self.ignored.len()
                )?;
                for ignored in &self.ignored {
                    writeln!(f, "{}", ignored)?;
                }
            }

            if let Some(disabled) = &self.disabled {
                writeln!(f, "{}", disabled)?;
            }

            Ok(())
        }
    }
}

impl fmt::Debug for Diagnostics<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            f.debug_struct(stringify!(Diagnostics))
                .field(stringify!(error), &self.error)
                .field(stringify!(ignored), &self.ignored)
                .field(stringify!(disabled), &self.disabled)
                .finish()
        } else {
            if let Some(error) = self.error(DiagnosticsTheme::Guess) {
                writeln!(f, "{}", error)?;
            }
            if let Some(warn) = self.warn(DiagnosticsTheme::Guess) {
                writeln!(f, "{}", warn)?;
            }
            Ok(())
        }
    }
}

#[derive(Debug)]
struct DiagnosticWithStr<'a> {
    diagnostic: &'a dyn Diagnostic,
    source: &'a str,
}

impl<'a> DiagnosticWithStr<'a> {
    fn new(diagnostic: &'a dyn Diagnostic, source: &'a str) -> Self {
        Self { diagnostic, source }
    }
}

impl fmt::Display for DiagnosticWithStr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.diagnostic, f)
    }
}

impl Error for DiagnosticWithStr<'_> {}

impl Diagnostic for DiagnosticWithStr<'_> {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.diagnostic.code()
    }

    fn severity(&self) -> Option<miette::Severity> {
        self.diagnostic.severity()
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.diagnostic.help()
    }

    fn url<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.diagnostic.url()
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&self.source)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        self.diagnostic.labels()
    }

    fn related<'a>(&'a self) -> Option<Box<dyn Iterator<Item = &'a dyn Diagnostic> + 'a>> {
        self.diagnostic.related()
    }
}
