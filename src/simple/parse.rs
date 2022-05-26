use {
    super::{Directive, Filter},
    crate::Diagnostics,
    miette::{Diagnostic, SourceSpan},
    std::str::FromStr,
    thiserror::Error,
    tracing_core::LevelFilter,
};

impl Filter {
    /// Parse a filter from its string representation.
    ///
    /// Filter compilation can produce warnings even when it succeeds, thus
    /// the nonstandard return type to provide [`Diagnostics`] on success.
    pub fn parse(spec: &str) -> (Option<Filter>, Option<Diagnostics<'_>>) {
        // this code is adapted directly from env_logger 0.9.0
        // env_logger is licensed under MIT OR Apache-2.0

        let recover_span = |substr: &str| {
            let offset = substr.as_ptr() as usize - spec.as_ptr() as usize;
            offset..offset + substr.len()
        };

        let mut directives = Vec::new();
        let mut parts = spec.split('/');
        let dirs = parts.next();
        let regex = parts.next();

        if let Some(after) = parts.next() {
            let regex = recover_span(regex.unwrap());
            let after = recover_span(after);
            let error = Error::MultipleSlash {
                slash: (after.start - 1..after.start).into(),
                regex: (regex.start..spec.len()).into(),
            };
            return (
                None,
                Some(Diagnostics {
                    error: Some(Box::new(error)),
                    ignored: Vec::new(),
                    disabled: None,
                    source: spec.into(),
                }),
            );
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
                                    span: recover_span(part1).into(),
                                });
                                continue;
                            },
                        },
                        (Some(_part0), Some(part1), Some(_part2)) => {
                            let part1 = recover_span(part1);
                            let dir = recover_span(dir);
                            warnings.push(Warning::InvalidLevel {
                                span: (part1.start..dir.end).into(),
                            });
                            continue;
                        },
                        _ => unreachable!(),
                    };
                let directive = Directive {
                    target: name.map(Into::into),
                    level: log_level,
                };
                let ix = directives.partition_point(|x| *x > directive);
                directives.insert(ix, directive);
            }
        }

        let regex = regex.and_then(|regex| match regex::Regex::new(regex) {
            Ok(regex) => Some(regex),
            Err(error) => {
                warnings.push(Warning::InvalidRegex {
                    error,
                    span: recover_span(regex).into(),
                });
                None
            },
        });

        let _ = regex; // mark used for cfg(not(feature = "regex"))
        let filter = Some(Filter { directives, regex });
        let report = if warnings.is_empty() {
            None
        } else {
            Some(Diagnostics {
                error: None,
                ignored: warnings
                    .into_iter()
                    .map(|x| Box::new(x) as Box<dyn Diagnostic + Send + Sync + 'static>)
                    .collect(),
                disabled: None,
                source: spec.into(),
            })
        };

        (filter, report)
    }
}

impl FromStr for Filter {
    type Err = Diagnostics<'static>;

    /// Parse a filter from its string representation, discarding warnings.
    fn from_str(spec: &str) -> Result<Self, Diagnostics<'static>> {
        let (filter, errs) = Self::parse(spec);
        filter.ok_or_else(|| {
            errs.expect("filter compilation failed without any diagnostics")
                .into_owned()
        })
    }
}

#[derive(Debug, Error, Diagnostic)]
#[diagnostic(severity(warning))]
enum Warning {
    #[error("invalid level filter specified")]
    #[diagnostic(help("valid level filters are OFF, ERROR, WARN, INFO, DEBUG, or TRACE"))]
    InvalidLevel {
        #[label]
        span: SourceSpan,
    },
    #[error("invalid regex specified")]
    #[diagnostic(code(tracing_filter::simple::InvalidRegex))]
    InvalidRegex {
        // no, we are not going to parse the formatted regex error
        // in order to translate it into miette span/labels
        // it'd be nice, but it's not worth the brittle hacks
        error: regex::Error,
        #[label("{}", .error)]
        span: SourceSpan,
    },
}

#[derive(Debug, Error, Diagnostic)]
#[diagnostic(severity(error))]
enum Error {
    #[error("logging spec has too many `/`s")]
    #[diagnostic(help("regex filters may not contain `/`"))]
    MultipleSlash {
        #[label("this `/` is not allowed ...")]
        slash: SourceSpan,
        #[label("... in this regex filter")]
        regex: SourceSpan,
    },
}
