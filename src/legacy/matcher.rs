use {
    crate::SmallVec,
    compact_str::CompactStr,
    matchers::Pattern,
    std::{
        cmp::Ordering,
        collections::HashMap,
        fmt,
        sync::atomic::{AtomicBool, Ordering::*},
    },
    tracing::{field::Visit, Metadata},
    tracing_core::{span, Field, LevelFilter},
};

#[derive(Debug)]
pub(super) struct MatchSet<T> {
    pub(super) fields: SmallVec<T>,
    pub(super) base_level: LevelFilter,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct FieldMatch {
    pub(super) name: CompactStr,
    pub(super) value: Option<ValueMatch>,
}

pub(super) type CallsiteMatcher = MatchSet<CallsiteMatch>;

#[derive(Debug)]
pub(super) struct CallsiteMatch {
    pub(super) fields: HashMap<Field, ValueMatch>,
    pub(super) level: LevelFilter,
}

pub(super) type SpanMatcher = MatchSet<SpanMatch>;

#[derive(Debug)]
pub(super) struct SpanMatch {
    fields: HashMap<Field, (ValueMatch, AtomicBool)>,
    level: LevelFilter,
    matched: AtomicBool,
}

#[derive(Debug, Clone)]
pub(super) enum ValueMatch {
    Bool(bool),
    F64(f64),
    U64(u64),
    I64(i64),
    NaN,
    Pat(Box<PatternMatch>),
}

#[derive(Debug, Clone)]
pub(super) struct PatternMatch {
    pub(super) matcher: Pattern,
    pub(super) pattern: CompactStr,
}

pub(super) trait Match {
    fn cares_about(&self, metadata: &Metadata<'_>) -> bool;
    fn level(&self) -> &LevelFilter;
}

impl CallsiteMatch {
    fn to_span_match(&self) -> SpanMatch {
        let fields = self
            .fields
            .iter()
            .map(|(k, v)| (k.clone(), (v.clone(), AtomicBool::new(false))))
            .collect();
        SpanMatch {
            fields,
            level: self.level,
            matched: AtomicBool::new(false),
        }
    }
}

impl CallsiteMatcher {
    /// Create a new `SpanMatch` for a given instance of the matched callsite.
    pub(super) fn to_span_matcher(&self, attrs: &span::Attributes<'_>) -> SpanMatcher {
        let fields = self
            .fields
            .iter()
            .map(|m| {
                let m = m.to_span_match();
                attrs.record(&mut m.visitor());
                m
            })
            .collect();
        SpanMatcher {
            fields,
            base_level: self.base_level,
        }
    }
}

impl SpanMatcher {
    /// Returns the level currently enabled for this callsite.
    pub(super) fn level(&self) -> LevelFilter {
        self.fields
            .iter()
            .filter_map(SpanMatch::filter)
            .max()
            .unwrap_or(self.base_level)
    }

    pub(super) fn record_update(&self, record: &span::Record<'_>) {
        for m in &self.fields {
            record.record(&mut m.visitor())
        }
    }
}

impl SpanMatch {
    fn visitor(&self) -> impl Visit + '_ {
        struct MatchVisitor<'a> {
            inner: &'a SpanMatch,
        }
        impl Visit for MatchVisitor<'_> {
            fn record_f64(&mut self, field: &Field, value: f64) {
                match self.inner.fields.get(field) {
                    Some((ValueMatch::NaN, matched)) if value.is_nan() => {
                        matched.store(true, Release);
                    },
                    Some((ValueMatch::F64(e), ref matched))
                        if (value - *e).abs() < std::f64::EPSILON =>
                    {
                        matched.store(true, Release);
                    },
                    _ => {},
                }
            }

            fn record_i64(&mut self, field: &Field, value: i64) {
                match self.inner.fields.get(field) {
                    Some((ValueMatch::I64(e), matched)) if value == *e => {
                        matched.store(true, Release);
                    },
                    Some((ValueMatch::U64(e), matched)) if Ok(value) == (*e).try_into() => {
                        matched.store(true, Release);
                    },
                    _ => {},
                }
            }

            fn record_u64(&mut self, field: &Field, value: u64) {
                match self.inner.fields.get(field) {
                    Some((ValueMatch::U64(e), matched)) if value == *e => {
                        matched.store(true, Release);
                    },
                    _ => {},
                }
            }

            fn record_bool(&mut self, field: &Field, value: bool) {
                match self.inner.fields.get(field) {
                    Some((ValueMatch::Bool(e), matched)) if value == *e => {
                        matched.store(true, Release);
                    },
                    _ => {},
                }
            }

            fn record_str(&mut self, field: &Field, value: &str) {
                match self.inner.fields.get(field) {
                    Some((ValueMatch::Pat(e), matched)) if e.matcher.matches(&value) => {
                        matched.store(true, Release);
                    },
                    _ => {},
                }
            }

            fn record_debug(&mut self, field: &Field, value: &dyn core::fmt::Debug) {
                match self.inner.fields.get(field) {
                    Some((ValueMatch::Pat(e), matched)) if e.matcher.debug_matches(&value) => {
                        matched.store(true, Release);
                    },
                    _ => {},
                }
            }
        }
        MatchVisitor { inner: self }
    }

    fn filter(&self) -> Option<LevelFilter> {
        if self.is_matched() {
            Some(self.level)
        } else {
            None
        }
    }

    fn is_matched(&self) -> bool {
        if self.matched.load(Acquire) {
            return true;
        }
        self.is_matched_slow()
    }

    #[inline(never)]
    fn is_matched_slow(&self) -> bool {
        let matched = self
            .fields
            .values()
            .all(|(_, matched)| matched.load(Acquire));
        if matched {
            self.matched.store(true, Release);
        }
        matched
    }
}

impl PartialOrd for FieldMatch {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

impl Ord for FieldMatch {
    fn cmp(&self, other: &Self) -> Ordering {
        // Ordering for `FieldMatch` directives is based first on _whether_ a
        // value is matched or not. This is semantically meaningful --- we would
        // prefer to check directives that match values first as they are more
        // specific.
        let has_value = match (self.value.as_ref(), other.value.as_ref()) {
            (Some(_), None) => Ordering::Greater,
            (None, Some(_)) => Ordering::Less,
            _ => Ordering::Equal,
        };
        // If both directives match a value, we fall back to the field names in
        // length + lexicographic ordering, and if these are equal as well, we
        // compare the match directives.
        //
        // This ordering is no longer semantically meaningful but is necessary
        // so that the directives can be sorted in a defined order.
        has_value
            .then_with(|| self.name.cmp(&other.name))
            .then_with(|| self.value.cmp(&other.value))
    }
}

impl Eq for ValueMatch {}
impl PartialEq for ValueMatch {
    fn eq(&self, other: &Self) -> bool {
        use ValueMatch::*;
        match (self, other) {
            (Bool(a), Bool(b)) => a.eq(b),
            (F64(a), F64(b)) => {
                debug_assert!(!a.is_nan());
                debug_assert!(!b.is_nan());

                a.eq(b)
            },
            (U64(a), U64(b)) => a.eq(b),
            (I64(a), I64(b)) => a.eq(b),
            (NaN, NaN) => true,
            (Pat(a), Pat(b)) => a.eq(b),
            _ => false,
        }
    }
}

impl PartialOrd for ValueMatch {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

impl Ord for ValueMatch {
    fn cmp(&self, rhs: &Self) -> std::cmp::Ordering {
        use ValueMatch::*;
        match (self, rhs) {
            (Bool(this), Bool(that)) => this.cmp(that),
            (Bool(_), _) => Ordering::Less,

            (F64(this), F64(that)) => this
                .partial_cmp(that)
                .expect("`ValueMatch::F64` may not contain `NaN` values"),
            (F64(_), Bool(_)) => Ordering::Greater,
            (F64(_), _) => Ordering::Less,

            (NaN, NaN) => Ordering::Equal,
            (NaN, Bool(_)) | (NaN, F64(_)) => Ordering::Greater,
            (NaN, _) => Ordering::Less,

            (U64(this), U64(that)) => this.cmp(that),
            (U64(_), Bool(_)) | (U64(_), F64(_)) | (U64(_), NaN) => Ordering::Greater,
            (U64(_), _) => Ordering::Less,

            (I64(this), I64(that)) => this.cmp(that),
            (I64(_), Bool(_)) | (I64(_), F64(_)) | (I64(_), NaN) | (I64(_), U64(_)) => {
                Ordering::Greater
            },
            (I64(_), _) => Ordering::Less,

            (Pat(this), Pat(that)) => this.cmp(that),
            (Pat(_), _) => Ordering::Greater,
        }
    }
}

impl Eq for PatternMatch {}
impl PartialEq for PatternMatch {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern
    }
}

impl PartialOrd for PatternMatch {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

impl Ord for PatternMatch {
    fn cmp(&self, rhs: &Self) -> Ordering {
        self.pattern.cmp(&rhs.pattern)
    }
}

impl fmt::Display for FieldMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.name, f)?;
        if let Some(ref value) = self.value {
            write!(f, "={}", value)?;
        }
        Ok(())
    }
}

impl fmt::Display for ValueMatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueMatch::Bool(ref inner) => fmt::Display::fmt(inner, f),
            ValueMatch::F64(ref inner) => fmt::Display::fmt(inner, f),
            ValueMatch::NaN => fmt::Display::fmt(&std::f64::NAN, f),
            ValueMatch::I64(ref inner) => fmt::Display::fmt(inner, f),
            ValueMatch::U64(ref inner) => fmt::Display::fmt(inner, f),
            ValueMatch::Pat(ref inner) => fmt::Display::fmt(&inner.pattern, f),
        }
    }
}
