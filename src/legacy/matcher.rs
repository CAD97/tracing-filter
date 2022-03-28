use {
    crate::FilterVec,
    matchers::Pattern,
    smartstring::alias::String,
    std::{
        collections::HashMap,
        sync::atomic::{AtomicBool, Ordering::*},
    },
    tracing::{field::Visit, Metadata},
    tracing_core::{span, Field, LevelFilter},
};

pub(super) struct MatchSet<T> {
    pub(super) fields: FilterVec<T>,
    pub(super) base_level: LevelFilter,
}

pub(super) struct FieldMatch {
    pub(super) name: String,
    pub(super) value: Option<ValueMatch>,
}

pub(super) type CallsiteMatcher = MatchSet<CallsiteMatch>;
pub(super) struct CallsiteMatch {
    pub(super) fields: HashMap<Field, ValueMatch>,
    pub(super) level: LevelFilter,
}

pub(super) type SpanMatcher = MatchSet<SpanMatch>;
pub(super) struct SpanMatch {
    fields: HashMap<Field, (ValueMatch, AtomicBool)>,
    level: LevelFilter,
    matched: AtomicBool,
}

#[derive(Clone)]
pub(super) enum ValueMatch {
    Bool(bool),
    F64(f64),
    U64(u64),
    I64(i64),
    NaN,
    Pat(Box<Pattern>),
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
                    Some((ValueMatch::Pat(e), matched)) if e.matches(&value) => {
                        matched.store(true, Release);
                    },
                    _ => {},
                }
            }

            fn record_debug(&mut self, field: &Field, value: &dyn core::fmt::Debug) {
                match self.inner.fields.get(field) {
                    Some((ValueMatch::Pat(e), matched)) if e.debug_matches(&value) => {
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
