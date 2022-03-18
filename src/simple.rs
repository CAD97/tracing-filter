use {
    miette::{Diagnostic, ErrReport, SourceSpan},
    smartstring::alias::String as SmartString,
    std::{fmt, ops::Range, str::FromStr},
    thiserror::Error,
    tracing_core::LevelFilter,
};

#[derive(Debug)]
pub struct Filter {
    directives: Vec<Directive>,
    #[cfg(all(feature = "regex"))]
    regex: Option<regex::Regex>,
}

#[derive(Debug)]
struct Directive {
    target: Option<SmartString>,
    level: LevelFilter,
}

impl Filter {
    /// Parse a filter from its string representation.
    ///
    /// Filter compilation can produce warnings even when it succeeds,
    /// thus the nonstandard return type to provide an [`ErrReport`] on success.
    pub fn parse(spec: impl AsRef<str> + Into<String>) -> (Option<Filter>, Option<ErrReport>) {
        let (filter, errs) = Self::parse_inner(spec.as_ref());
        let errs = errs.map(|errs| errs.with_source_code(spec.into()));
        (filter, errs)
    }

    fn parse_inner(spec: &str) -> (Option<Filter>, Option<ErrReport>) {
        // this code is adapted directly from env_logger 0.9.0
        // env_logger is licensed under MIT OR Apache-2.0

        fn recover_span(spec: &str, substr: &str) -> Range<usize> {
            let offset = substr.as_ptr() as usize - spec.as_ptr() as usize;
            offset..offset + substr.len()
        }

        let mut directives = Vec::new();
        let mut parts = spec.split('/');
        let dirs = parts.next();
        let regex = parts.next();

        if let Some(after) = parts.next() {
            let regex = recover_span(spec, regex.unwrap());
            let after = recover_span(spec, after);
            let error = Error::MultipleSlash {
                slash: (after.start - 1..after.start).into(),
                regex: (regex.start..spec.len()).into(),
            };
            return (None, Some(error.into()));
        }

        let mut warnings = Vec::new();

        if let Some(dirs) = dirs {
            for dir in dirs.split(',').map(|dir| dir.trim()) {
                if dir.is_empty() {
                    continue;
                }
                let mut parts = dir.split('=');
                let (log_level, name) =
                    match (parts.next(), parts.next().map(str::trim), parts.next()) {
                        (Some(part0), None, None) => {
                            // if the single argument is a log-level string
                            // or number, treat that as a global fallback
                            match part0.parse() {
                                Ok(num) => (num, None),
                                Err(_) => (LevelFilter::TRACE, Some(part0)),
                            }
                        },
                        (Some(part0), Some(""), None) => (LevelFilter::TRACE, Some(part0)),
                        (Some(part0), Some(part1), None) => match part1.parse() {
                            Ok(num) => (num, Some(part0)),
                            _ => {
                                warnings.push(Warning::InvalidLevel {
                                    span: recover_span(spec, part1).into(),
                                });
                                continue;
                            },
                        },
                        (Some(part0), Some(part1), Some(_part2)) => {
                            let part0 = recover_span(spec, part0);
                            let part1 = recover_span(spec, part1);
                            warnings.push(Warning::InvalidTarget {
                                equals: (part1.start - 1..part1.start).into(),
                                target: (part0.start..part1.end).into(),
                            });
                            continue;
                        },
                        _ => unreachable!(),
                    };
                directives.push(Directive {
                    target: name.map(Into::into),
                    level: log_level,
                })
            }
        }

        let regex = regex.and_then(|regex| {
            #[cfg(feature = "regex")]
            {
                match regex::Regex::new(regex) {
                    Ok(regex) => Some(regex),
                    Err(error) => {
                        warnings.push(Warning::InvalidRegex {
                            error,
                            span: recover_span(spec, regex).into(),
                        });
                        None
                    },
                }
            }

            #[cfg(not(feature = "regex"))]
            {
                warnings.push(Warning::DisabledRegex {
                    span: recover_span(spec, regex).into(),
                });
                None::<()>
            }
        });

        let _ = regex; // mark used for cfg(not(feature = "regex"))
        let filter = Some(Filter {
            directives,
            #[cfg(feature = "regex")]
            regex,
        });
        let report = if warnings.is_empty() {
            None
        } else {
            Some(Warnings { warnings }.into())
        };

        (filter, report)
    }
}

impl FromStr for Filter {
    type Err = ErrReport;

    /// Parse a filter from its string representation, discarding warnings.
    fn from_str(spec: &str) -> miette::Result<Self> {
        let (filter, errs) = Self::parse(spec);
        filter.ok_or_else(|| errs.expect("filter compilation failed without any diagnostics"))
    }
}

impl fmt::Display for Filter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for directive in &self.directives {
            if let Some(target) = &directive.target {
                write!(f, "{}=", target)?;
            }
            write!(f, "{},", directive.level)?;
        }

        #[cfg(feature = "regex")]
        if let Some(regex) = &self.regex {
            write!(f, "/{}", regex)?;
        }

        Ok(())
    }
}

#[derive(Debug, Error, Diagnostic)]
#[error("{} directives were ignored as invalid", .warnings.len())]
#[diagnostic(severity(warning))]
struct Warnings {
    #[related]
    warnings: Vec<Warning>,
}

#[derive(Debug, Error, Diagnostic)]
#[diagnostic(severity(warning))]
pub enum Warning {
    #[error("regex filter used, but regex filters are not enabled")]
    #[diagnostic(
        code(tracing_filter::simple::Warning::DisabledRegex),
        url(docsrs),
        help("enable the `regex` filter for `tracing_filter` to enable")
    )]
    DisabledRegex {
        #[label("regex filter used here")]
        span: SourceSpan,
    },
    #[error("invalid level directive specified")]
    #[diagnostic(
        code(tracing_filter::simple::Warning::InvalidLevel),
        url(docsrs),
        help("valid directives are OFF, ERROR, WARN, INFO, DEBUG, or TRACE")
    )]
    InvalidLevel {
        #[label("this level is invalid")]
        span: SourceSpan,
    },
    #[error("invalid target directive specified")]
    #[diagnostic(code(tracing_filter::simple::Warning::InvalidTarget), url(docsrs))]
    InvalidTarget {
        #[label("this `=` is not allowed ...")]
        equals: SourceSpan,
        #[label("... in this target directive")]
        target: SourceSpan,
    },
    #[cfg(feature = "regex")]
    #[error("invalid regex specified")]
    #[diagnostic(code(tracing_filter::simple::Warning::InvalidRegex), url(docsrs))]
    InvalidRegex {
        error: regex::Error,
        #[label("{}", .error)]
        span: SourceSpan,
    },
}

#[derive(Debug, Error, Diagnostic)]
#[diagnostic(severity(error))]
pub enum Error {
    #[error("logging spec has too many `/`s")]
    #[diagnostic(
        code(tracing_filter::simple::Error::MultipleSlash),
        url(docsrs),
        help("regex filters may not contain `/`")
    )]
    MultipleSlash {
        #[label("this `/` is not allowed ...")]
        slash: SourceSpan,
        #[label("... in this regex filter")]
        regex: SourceSpan,
    },
}
