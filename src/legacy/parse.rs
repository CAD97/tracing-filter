use crate::legacy::matcher::PatternMatch;

use super::directive::StaticDirective;

use {
    super::{
        directive::DynamicDirective,
        matcher::{FieldMatch, ValueMatch},
        Filter,
    },
    crate::SmallVec,
    miette::{Diagnostic, ErrReport, SourceSpan},
    once_cell::sync::Lazy,
    regex::Regex,
    std::ops::Range,
    thiserror::Error,
    tracing::level_filters::STATIC_MAX_LEVEL,
    tracing_core::{Level, LevelFilter},
};

impl Filter {
    /// Parse a filter from its string representation.
    ///
    /// Filter compilation can produce warnings even when it succeeds,
    /// thus the nonstandard return type to provide an [`ErrReport`] on success.
    pub fn parse(spec: impl AsRef<str> + Into<String>) -> (Filter, Option<ErrReport>) {
        let (filter, errs) = Self::parse_inner(spec.as_ref());
        let errs = errs.map(|errs| errs.with_source_code(spec.into()));
        (filter, errs)
    }

    fn parse_inner(spec: &str) -> (Filter, Option<ErrReport>) {
        let mut directives = Vec::new();
        let mut ignored = Vec::new();

        let mut i = 0;
        while i < spec.len() {
            let j = spec[i..].find(',').map(|j| i + j).unwrap_or(spec.len());
            match DynamicDirective::parse(spec, i..j) {
                Ok(directive) => directives.push(directive),
                Err(directive) => ignored.push(directive),
            }
            i = j + 1;
        }

        let ignored = Some(ignored)
            .filter(|v| !v.is_empty())
            .map(IgnoredDirectives);
        let (filter, disabled) = Self::from_directives(directives);
        match (ignored, disabled) {
            (None, None) => (filter, None),
            (ignored, disabled) => (filter, Some(Warnings { ignored, disabled }.into())),
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
                        .map(|t| format!("the `{t}` target"))
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

        if statics.directives.is_empty() && !dynamics.directives.is_empty() {
            statics.add(StaticDirective::default());
        }

        let filter = Filter {
            scope: Default::default(),
            statics,
            dynamics,
            by_id: Default::default(),
            by_cs: Default::default(),
        };
        (filter, advice)
    }
}

impl DynamicDirective {
    fn parse(src: &str, range: Range<usize>) -> Result<Self, IgnoredDirective> {
        let spec = &src[range.start..range.end];

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

        let mut pos = 0;

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
        let mut parse_target_span = |pos: &mut usize| -> Result<(), IgnoredDirective> {
            if let Some(m) = TARGET_RE.find(&spec[*pos..]) {
                // target
                debug_assert_eq!(m.start(), 0);
                target = Some(spec[*pos..][..m.end()].into());
                *pos += m.end();
            } else if spec[*pos..].starts_with('[') {
                // span
                let span_start = spec[*pos..]
                    .find(|c: char| c != '[')
                    .map(|p| *pos + p)
                    .unwrap_or(spec.len());
                match spec[span_start..].find(']') {
                    Some(span_len) => {
                        let m = SPAN_RE.captures(&spec[span_start..][..span_len]).unwrap();
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
                                                span: (range.start + span_start + m.start()
                                                    ..range.start + span_start + m.end())
                                                    .into(),
                                            }
                                        })
                                    })
                                    .collect::<Result<SmallVec<_>, _>>()
                            })
                            .transpose()?
                            .unwrap_or_default();
                        *pos = span_start + span_len + 1;
                    },
                    None => {
                        return Err(IgnoredDirective::UnclosedSpan {
                            open: (range.start + span_start..range.start + span_start + 1).into(),
                            close: (range.end..range.end).into(),
                        })
                    },
                }
            } else if first_time {
                return Err(IgnoredDirective::InvalidTarget {
                    span: (range.start + *pos..range.end).into(),
                });
            }

            first_time = false;
            Ok(())
        };

        parse_target_span(&mut pos)?;
        if !spec[pos..].starts_with('=') {
            parse_target_span(&mut pos)?;
        }

        // level or nothing
        // (?:=(?P<level>(?i:trace|debug|info|warn|error|off|[0-5]))?)?$
        if spec[pos..].starts_with('=') {
            pos += 1;
            if pos == spec.len() {
                Ok(DynamicDirective {
                    span,
                    fields,
                    target,
                    level: LevelFilter::TRACE,
                })
            } else {
                match spec[pos..].parse() {
                    Ok(level) => Ok(DynamicDirective {
                        span,
                        fields,
                        target,
                        level,
                    }),
                    Err(_) if pos == range.end => Ok(DynamicDirective {
                        span,
                        fields,
                        target,
                        level: LevelFilter::TRACE,
                    }),
                    Err(_) => Err(IgnoredDirective::InvalidLevel {
                        span: (range.start + pos..range.end).into(),
                    }),
                }
            }
        } else if spec.len() == pos {
            Ok(DynamicDirective {
                span,
                fields,
                target,
                level: LevelFilter::TRACE,
            })
        } else {
            Err(IgnoredDirective::InvalidTrailing {
                span: (range.start + pos..range.end).into(),
            })
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
#[error("some directives had no effect")]
#[diagnostic(severity(error))]
struct Warnings {
    #[related]
    ignored: Option<IgnoredDirectives>,
    #[related]
    disabled: Option<DisabledDirectives>,
}

#[derive(Debug, Error, Diagnostic)]
#[error("{} directives were ignored as invalid", .0.len())]
#[diagnostic(severity(warning))]
struct IgnoredDirectives(#[related] Vec<IgnoredDirective>);

#[derive(Debug, Error, Diagnostic)]
#[diagnostic(severity(warning))]
pub enum IgnoredDirective {
    #[error("invalid target specified")]
    #[diagnostic(code(tracing_filter::legacy::InvalidTarget), url(docsrs))]
    InvalidTarget {
        #[label]
        span: SourceSpan,
    },
    #[error("invalid level filter specified")]
    #[diagnostic(
        code(tracing_filter::legacy::InvalidLevel),
        url(docsrs),
        help("valid level filters are OFF, ERROR, WARN, INFO, DEBUG, or TRACE")
    )]
    InvalidLevel {
        #[label]
        span: SourceSpan,
    },
    #[error("invalid regex specified")]
    #[diagnostic(code(tracing_filter::legacy::InvalidRegex), url(docsrs))]
    InvalidRegex {
        // no, we are not going to parse the formatted regex error
        // in order to translate it into miette span/labels
        // it'd be nice, but it's not worth the brittle hacks
        error: matchers::Error,
        #[label("{}", .error)]
        span: SourceSpan,
    },
    #[error("invalid trailing characters")]
    #[diagnostic(code(tracing_filter::legacy::InvalidTrailing), url(docsrs))]
    InvalidTrailing {
        #[label]
        span: SourceSpan,
    },
    #[error("unclosed span directive")]
    #[diagnostic(code(tracing_filter::legacy::UnclosedSpan), url(docsrs))]
    UnclosedSpan {
        #[label("opened here")]
        open: SourceSpan,
        #[label("stopped looking here")]
        close: SourceSpan,
    },
}

#[derive(Debug, Error, Diagnostic)]
#[diagnostic(severity(warning))]
#[error("{} directives would enabled traces that are disabled statically", .directives.len())]
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
    help("to enable {}logging, remove the `{}` feature", .earlier_level.map(|l| format!("{l} ")).unwrap_or_default(), .feature)
)]
#[error("the static max level is `{}`", .static_level)]
struct StaticMaxAdvice {
    static_level: LevelFilter,
    earlier_level: Option<Level>,
    feature: &'static str,
}
