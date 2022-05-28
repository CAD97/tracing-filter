use {
    super::{
        directive::{DynamicDirective, StaticDirective},
        matcher::{FieldMatch, PatternMatch, ValueMatch},
        Filter,
    },
    crate::{Diagnostics, SmallVec},
    miette::{Diagnostic, SourceSpan},
    once_cell::sync::Lazy,
    regex::Regex,
    std::{ops::Range, str::FromStr},
    thiserror::Error,
    tracing::level_filters::STATIC_MAX_LEVEL,
    tracing_core::{Level, LevelFilter},
};

impl FromStr for Filter {
    type Err = Diagnostics<'static>;

    /// Parse a filter from its string representation, discarding warnings.
    fn from_str(s: &str) -> Result<Self, Diagnostics<'static>> {
        let (filter, errs) = Self::parse(s);
        if let Some(errs) = errs {
            Err(errs.into_owned())
        } else {
            Ok(filter)
        }
    }
}

impl Filter {
    /// Parse a filter from its string representation.
    ///
    /// Filter compilation can produce warnings even when it succeeds, thus
    /// the nonstandard return type to provide [`Diagnostics`] on success.
    pub fn parse(spec: &str) -> (Filter, Option<Diagnostics<'_>>) {
        let recover_span = |substr: &str| {
            let offset = substr.as_ptr() as usize - spec.as_ptr() as usize;
            offset..offset + substr.len()
        };

        let mut directives = Vec::new();
        let mut ignored = Vec::new();

        for directive_spec in spec.split(',') {
            match DynamicDirective::parse(directive_spec, recover_span) {
                Ok(directive) => directives.push(directive),
                Err(directive) => ignored.push(directive),
            }
        }

        let ignored: Vec<_> = ignored
            .into_iter()
            .map(|x| Box::new(x) as Box<dyn Diagnostic + Send + Sync + 'static>)
            .collect();
        let (filter, disabled) = Self::from_directives(directives);
        match (&*ignored, disabled) {
            (&[], None) => (filter, None),
            (_, disabled) => (
                filter,
                Some(Diagnostics {
                    error: None,
                    ignored,
                    disabled: disabled
                        .map(|x| Box::new(x) as Box<dyn Diagnostic + Send + Sync + 'static>),
                    source: spec.into(),
                }),
            ),
        }
    }

    fn from_directives(directives: Vec<DynamicDirective>) -> (Filter, Option<DisabledDirectives>) {
        let disabled: Vec<_> = directives
            .iter()
            .filter(|directive| directive.level > STATIC_MAX_LEVEL)
            .collect();

        let advice = if !disabled.is_empty() {
            let mut disabled_advice = Vec::new();

            for directive in disabled {
                disabled_advice.push(DisabledDirective {
                    directive: format!("{}", directive),
                    level: directive.level.into_level().unwrap(),
                    target: directive
                        .target
                        .as_deref()
                        .map(|t| format!("the `{}` target", t))
                        .unwrap_or_else(|| "all targets".into()),
                });
            }

            let (feature, earlier_level) = match STATIC_MAX_LEVEL.into_level() {
                Some(Level::TRACE) => unreachable!(),
                Some(Level::DEBUG) => ("max_level_debug", Some(Level::TRACE)),
                Some(Level::INFO) => ("max_level_info", Some(Level::DEBUG)),
                Some(Level::WARN) => ("max_level_warn", Some(Level::INFO)),
                Some(Level::ERROR) => ("max_level_error", Some(Level::WARN)),
                None => ("max_level_off", None),
            };
            let static_max = StaticMaxAdvice {
                static_level: STATIC_MAX_LEVEL,
                earlier_level,
                feature,
            };
            Some(DisabledDirectives {
                directives: disabled_advice,
                static_max: Some(static_max),
            })
        } else {
            None
        };

        let (dynamics, mut statics) = DynamicDirective::make_tables(directives);

        if statics.directives.is_empty() && dynamics.directives.is_empty() {
            statics.add(StaticDirective::default());
        }

        let filter = Filter {
            scope: Default::default(),
            statics,
            dynamics,
            by_cs: Default::default(),
        };
        (filter, advice)
    }
}

impl FromStr for DynamicDirective {
    type Err = Diagnostics<'static>;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s, |substr: &str| {
            let offset = substr.as_ptr() as usize - s.as_ptr() as usize;
            offset..offset + substr.len()
        })
        .map_err(|ignored| {
            Diagnostics {
                error: None,
                ignored: vec![Box::new(ignored)],
                disabled: None,
                source: s.into(),
            }
            .into_owned()
        })
    }
}

impl DynamicDirective {
    fn parse(
        mut spec: &str,
        recover_span: impl Fn(&str) -> Range<usize>,
    ) -> Result<Self, IgnoredDirective> {
        // if it can be a global level filter, it is
        // ^(?P<global_level>(?i:trace|debug|info|warn|error|off|[0-5]))$
        if let Ok(level) = spec.parse() {
            return Ok(DynamicDirective {
                level,
                span: None,
                fields: SmallVec::new(),
                target: None,
            });
        }

        // target and span parts are order insignificant
        // ^(?:(?P<target>[\w:-]+)|(?P<span>\[[^\]]*\])){1,2}
        let mut span = None;
        let mut fields = SmallVec::new();
        let mut target = None;

        // This is the simplest way to check the \w character class; otherwise
        // \p{Alphabetic} + \p{Nd} + \p{M} + \p{Pc} + \p{Join_Control}
        // char::is_alphanumeric() + ????? + ?????? + ????????????????
        static TARGET_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[\w:-]+").unwrap());

        // The semantics of these are way too crazy to implement by hand :(
        // Notably, the matches aren't anchored, meaning they can be partial.
        // Just using the regex engine is the best way to maintain compatibilty.
        static SPAN_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"(?P<name>[^\]\{]+)?(?:\{(?P<fields>[^\}]*)\})?").unwrap());
        static FIELDS_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"[[:word:]][[[:word:]]\.]*(?:=[^,]+)?(?:,|$)").unwrap());

        let mut first_time = true;
        let mut parse_target_span = |spec: &mut &str| -> Result<(), IgnoredDirective> {
            if let Some(m) = TARGET_RE.find(spec) {
                // target
                debug_assert_eq!(m.start(), 0);
                target = Some(m.as_str().into());
                *spec = &spec[m.end()..];
            } else if spec.starts_with('[') {
                // span
                *spec = spec.trim_start_matches('['); // yes, this is upstream behavior
                match spec.split_once(']') {
                    Some((span_spec, rest)) => {
                        let m = SPAN_RE.captures(span_spec).unwrap();
                        span = m.name("name").map(|m| m.as_str().into());
                        fields = m
                            .name("fields")
                            .map(|m| {
                                FIELDS_RE
                                    .find_iter(m.as_str())
                                    .map(|m| {
                                        FieldMatch::parse(m.as_str()).map_err(|error| {
                                            IgnoredDirective::InvalidRegex {
                                                error,
                                                span: recover_span(
                                                    // ugly but correct
                                                    m.as_str().split('=').nth(1).unwrap(),
                                                )
                                                .into(),
                                            }
                                        })
                                    })
                                    .collect::<Result<SmallVec<_>, _>>()
                            })
                            .transpose()?
                            .unwrap_or_default();
                        *spec = rest;
                    },
                    None => {
                        let spec = recover_span(spec);
                        return Err(IgnoredDirective::UnclosedSpan {
                            open: (spec.start - 1..spec.start).into(),
                            close: (spec.end..spec.end).into(),
                        });
                    },
                }
            } else if first_time {
                return Err(IgnoredDirective::InvalidTarget {
                    span: recover_span(spec).into(),
                });
            }

            first_time = false;
            Ok(())
        };

        parse_target_span(&mut spec)?;
        if !spec.starts_with('=') {
            parse_target_span(&mut spec)?;
        }

        // level or nothing
        // (?:=(?P<level>(?i:trace|debug|info|warn|error|off|[0-5]))?)?$
        match spec {
            "" | "=" => Ok(DynamicDirective {
                span,
                fields,
                target,
                level: LevelFilter::TRACE,
            }),
            _ if spec.starts_with('=') => {
                let spec = &spec[1..];
                match spec.parse() {
                    Ok(level) => Ok(DynamicDirective {
                        span,
                        fields,
                        target,
                        level,
                    }),
                    Err(_) => Err(IgnoredDirective::InvalidLevel {
                        span: recover_span(spec).into(),
                    }),
                }
            },
            _ => Err(IgnoredDirective::InvalidTrailing {
                span: recover_span(spec).into(),
            }),
        }
    }
}

impl FieldMatch {
    fn parse(s: &str) -> Result<Self, matchers::Error> {
        let mut split = s.split('=');
        Ok(FieldMatch {
            name: split.next().unwrap_or_default().into(),
            value: split.next().map(ValueMatch::parse).transpose()?,
        })
    }
}

impl ValueMatch {
    fn parse(s: &str) -> Result<Self, matchers::Error> {
        fn value_match_f64(v: f64) -> ValueMatch {
            if v.is_nan() {
                ValueMatch::NaN
            } else {
                ValueMatch::F64(v)
            }
        }

        Err(())
            .or_else(|_| s.parse().map(ValueMatch::Bool))
            .or_else(|_| s.parse().map(ValueMatch::U64))
            .or_else(|_| s.parse().map(ValueMatch::I64))
            .or_else(|_| s.parse().map(value_match_f64))
            .or_else(|_| {
                s.parse()
                    .map(|matcher| PatternMatch {
                        matcher,
                        pattern: s.into(),
                    })
                    .map(Box::new)
                    .map(ValueMatch::Pat)
            })
    }
}

#[derive(Debug, Error, Diagnostic)]
#[error("{} directives were ignored as invalid", .0.len())]
#[diagnostic(severity(warning))]
struct IgnoredDirectives(#[related] Vec<IgnoredDirective>);

#[derive(Debug, Error, Diagnostic)]
#[diagnostic(severity(warning))]
enum IgnoredDirective {
    #[error("invalid target specified")]
    InvalidTarget {
        #[label]
        span: SourceSpan,
    },
    #[error("invalid level filter specified")]
    #[diagnostic(help("valid level filters are OFF, ERROR, WARN, INFO, DEBUG, or TRACE"))]
    InvalidLevel {
        #[label]
        span: SourceSpan,
    },
    #[error("invalid regex specified")]
    InvalidRegex {
        // no, we are not going to parse the formatted regex error
        // in order to translate it into miette span/labels
        // it'd be nice, but it's not worth the brittle hacks
        error: matchers::Error,
        #[label("{}", .error)]
        span: SourceSpan,
    },
    #[error("invalid trailing characters")]
    InvalidTrailing {
        #[label]
        span: SourceSpan,
    },
    #[error("unclosed span directive")]
    UnclosedSpan {
        #[label("opened here")]
        open: SourceSpan,
        #[label("stopped looking here")]
        close: SourceSpan,
    },
}

#[derive(Debug, Error, Diagnostic)]
#[diagnostic(severity(warning))]
#[error("{} directives would enable traces that are disabled statically", .directives.len())]
struct DisabledDirectives {
    #[related]
    directives: Vec<DisabledDirective>,
    #[related]
    static_max: Option<StaticMaxAdvice>,
}

#[derive(Debug, Error, Diagnostic)]
#[diagnostic(severity(warning))]
#[error("`{}` would enabled the {} level for {}", .directive, .level, .target)]
struct DisabledDirective {
    directive: String,
    level: Level,
    target: String,
}

#[derive(Debug, Error, Diagnostic)]
#[diagnostic(
    severity(advice),
    help(
        "to enable {}logging, remove the `{}` feature",
        .earlier_level.map(|l| format!("{} ", l)).unwrap_or_default(),
        .feature
    ),
)]
#[error("the static max level is `{}`", .static_level)]
struct StaticMaxAdvice {
    static_level: LevelFilter,
    earlier_level: Option<Level>,
    feature: &'static str,
}
