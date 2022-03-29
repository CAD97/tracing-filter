use {
    super::matcher::{CallsiteMatch, CallsiteMatcher, FieldMatch, Match},
    crate::SmallVec,
    smartstring::alias::String,
    sorted_vec::SortedSet,
    std::{cmp::Ordering, collections::HashMap, fmt},
    tracing::Metadata,
    tracing_core::LevelFilter,
};

pub(super) struct DirectiveSet<T: Ord> {
    pub(super) directives: SortedSet<T>,
    pub(super) level: LevelFilter,
}

pub(super) type Dynamics = DirectiveSet<DynamicDirective>;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct DynamicDirective {
    pub(super) span: Option<String>,
    pub(super) fields: SmallVec<FieldMatch>,
    pub(super) target: Option<String>,
    pub(super) level: LevelFilter,
}

pub(super) type Statics = DirectiveSet<StaticDirective>;

#[derive(PartialEq, Eq)]
pub(super) struct StaticDirective {
    pub(super) target: Option<String>,
    pub(super) fields: SmallVec<String>,
    pub(super) level: LevelFilter,
}

impl<T: Ord> DirectiveSet<T> {
    fn directives(&self) -> impl Iterator<Item = &T> {
        self.directives.iter()
    }

    fn directives_for<'a>(&'a self, metadata: &'a Metadata<'a>) -> impl Iterator<Item = &'a T> + 'a
    where
        T: Match,
    {
        self.directives().filter(move |d| d.cares_about(metadata))
    }

    pub(super) fn add(&mut self, directive: T)
    where
        T: Match,
    {
        // does this directive enable a more verbose level than the current
        // max? if so, update the max level.
        let level = *directive.level();
        if level > self.level {
            self.level = level;
        }
        // insert the directive into the vec of directives, ordered by
        // specificity (length of target + number of field filters). this
        // ensures that, when finding a directive to match a span or event, we
        // search the directive set in most specific first order.
        self.directives.insert(directive);
    }
}

impl<T: Match + Ord> FromIterator<T> for DirectiveSet<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut this = Self::default();
        for directive in iter.into_iter() {
            this.add(directive)
        }
        this
    }
}

impl<T: Ord> Default for DirectiveSet<T> {
    fn default() -> Self {
        Self {
            directives: SortedSet::new(),
            level: LevelFilter::OFF,
        }
    }
}

impl DynamicDirective {
    fn field_matcher(&self, metadata: &Metadata<'_>) -> Option<CallsiteMatch> {
        let fieldset = metadata.fields();
        let fields = self
            .fields
            .iter()
            .filter_map(
                |FieldMatch {
                     ref name,
                     ref value,
                 }| {
                    if let Some(field) = fieldset.field(name) {
                        let value = value.as_ref().cloned()?;
                        Some(Ok((field, value)))
                    } else {
                        Some(Err(()))
                    }
                },
            )
            .collect::<Result<HashMap<_, _>, ()>>()
            .ok()?;
        Some(CallsiteMatch {
            fields,
            level: self.level,
        })
    }
}

impl Match for DynamicDirective {
    fn cares_about(&self, metadata: &Metadata<'_>) -> bool {
        // Does this directive have a target filter, and does it match the
        // metadata's target?
        if let Some(ref target) = self.target {
            if !metadata.target().starts_with(&target[..]) {
                return false;
            }
        }

        // Do we have a name filter, and does it match the metadata's name?
        // TODO(eliza): put name globbing here?
        if let Some(ref name) = self.span {
            if name != metadata.name() {
                return false;
            }
        }

        // Does the metadata define all the fields that this directive cares about?
        let fields = metadata.fields();
        for field in &self.fields {
            if fields.field(&field.name).is_none() {
                return false;
            }
        }

        true
    }

    fn level(&self) -> &LevelFilter {
        &self.level
    }
}

impl DynamicDirective {
    fn is_dynamic(&self) -> bool {
        self.span.is_some() || !self.fields.is_empty()
    }

    fn is_static(&self) -> bool {
        self.span.is_none() && self.fields.iter().all(|field| field.value.is_none())
    }

    fn to_static(&self) -> Option<StaticDirective> {
        if self.is_static() {
            // TODO(eliza): these strings are all immutable; we should consider
            // using an O(1) clone smartstring to make this more efficient...
            Some(StaticDirective {
                target: self.target.clone(),
                fields: self.fields.iter().map(|field| field.name.clone()).collect(),
                level: self.level,
            })
        } else {
            None
        }
    }

    pub(super) fn make_tables(directives: Vec<DynamicDirective>) -> (Dynamics, Statics) {
        // TODO(eliza): this could be made more efficient...
        let (dynamics, statics): (Vec<DynamicDirective>, Vec<DynamicDirective>) = directives
            .into_iter()
            .partition(DynamicDirective::is_dynamic);
        let statics = statics
            .into_iter()
            .filter_map(|d| d.to_static())
            .chain(dynamics.iter().filter_map(DynamicDirective::to_static))
            .collect();
        (Dynamics::from_iter(dynamics), statics)
    }
}

impl Match for StaticDirective {
    fn cares_about(&self, metadata: &tracing::Metadata<'_>) -> bool {
        // Does this directive have a target filter, and does it match the
        // metadata's target?
        if let Some(ref target) = self.target {
            if !metadata.target().starts_with(&target[..]) {
                return false;
            }
        }

        if metadata.is_event() && !self.fields.is_empty() {
            let fields = metadata.fields();
            for name in &self.fields {
                if fields.field(name).is_none() {
                    return false;
                }
            }
        }

        true
    }

    fn level(&self) -> &LevelFilter {
        &self.level
    }
}

impl Statics {
    pub(super) fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        let level = metadata.level();
        if self.level >= *level {
            return false;
        }
        match self.directives_for(metadata).next() {
            Some(d) => d.level >= *level,
            None => false,
        }
    }
}

impl Dynamics {
    pub(super) fn matcher(&self, metadata: &Metadata<'_>) -> Option<CallsiteMatcher> {
        let mut level = None;
        let fields = self
            .directives_for(metadata)
            .filter_map(|d| {
                if let Some(f) = d.field_matcher(metadata) {
                    return Some(f);
                }
                match level {
                    Some(ref b) if d.level > *b => level = Some(d.level),
                    None => level = Some(d.level),
                    _ => {},
                }
                None
            })
            .collect();

        if let Some(level) = level {
            Some(CallsiteMatcher {
                fields,
                base_level: level,
            })
        } else if !fields.is_empty() {
            Some(CallsiteMatcher {
                fields,
                base_level: level.unwrap_or(LevelFilter::OFF),
            })
        } else {
            None
        }
    }
}

impl Default for StaticDirective {
    fn default() -> Self {
        Self {
            target: None,
            fields: SmallVec::new(),
            level: LevelFilter::ERROR,
        }
    }
}

impl PartialOrd for DynamicDirective {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DynamicDirective {
    fn cmp(&self, other: &DynamicDirective) -> Ordering {
        // We attempt to order directives by how "specific" they are. This
        // ensures that we try the most specific directives first when
        // attempting to match a piece of metadata.

        // First, we compare based on whether a target is specified, and the
        // lengths of those targets if both have targets.
        let ordering = self
            .target
            .as_ref()
            .map(String::len)
            .cmp(&other.target.as_ref().map(String::len))
            // Next compare based on the presence of span names.
            .then_with(|| self.span.is_some().cmp(&other.span.is_some()))
            // Then we compare how many fields are defined by each
            // directive.
            .then_with(|| self.fields.len().cmp(&other.fields.len()))
            // Finally, we fall back to lexicographical ordering if the directives are
            // equally specific. Although this is no longer semantically important,
            // we need to define a total ordering to determine the directive's place
            // in the BTreeMap.
            .then_with(|| {
                self.target
                    .cmp(&other.target)
                    .then_with(|| self.span.cmp(&other.span))
                    .then_with(|| self.fields[..].cmp(&other.fields[..]))
            })
            .reverse();

        #[cfg(debug_assertions)]
        {
            if ordering == Ordering::Equal {
                debug_assert_eq!(
                    self.target, other.target,
                    "invariant violated: Ordering::Equal must imply a.target == b.target"
                );
                debug_assert_eq!(
                    self.span, other.span,
                    "invariant violated: Ordering::Equal must imply a.span == b.span"
                );
                debug_assert_eq!(
                    self.fields, other.fields,
                    "invariant violated: Ordering::Equal must imply a.fields == b.fields"
                );
            }
        }

        ordering
    }
}

impl PartialOrd for StaticDirective {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for StaticDirective {
    fn cmp(&self, other: &StaticDirective) -> Ordering {
        // We attempt to order directives by how "specific" they are. This
        // ensures that we try the most specific directives first when
        // attempting to match a piece of metadata.

        // First, we compare based on whether a target is specified, and the
        // lengths of those targets if both have targets.
        let ordering = self
            .target
            .as_ref()
            .map(String::len)
            .cmp(&other.target.as_ref().map(String::len))
            // Then we compare how many field names are matched by each directive.
            .then_with(|| self.fields.len().cmp(&other.fields.len()))
            // Finally, we fall back to lexicographical ordering if the directives are
            // equally specific. Although this is no longer semantically important,
            // we need to define a total ordering to determine the directive's place
            // in the BTreeMap.
            .then_with(|| {
                self.target
                    .cmp(&other.target)
                    .then_with(|| self.fields[..].cmp(&other.fields[..]))
            })
            .reverse();

        #[cfg(debug_assertions)]
        {
            if ordering == Ordering::Equal {
                debug_assert_eq!(
                    self.target, other.target,
                    "invariant violated: Ordering::Equal must imply a.target == b.target"
                );
                debug_assert_eq!(
                    self.fields, other.fields,
                    "invariant violated: Ordering::Equal must imply a.fields == b.fields"
                );
            }
        }

        ordering
    }
}

impl fmt::Display for DynamicDirective {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut wrote_any = false;
        if let Some(ref target) = self.target {
            fmt::Display::fmt(target, f)?;
            wrote_any = true;
        }

        if self.span.is_some() || !self.fields.is_empty() {
            f.write_str("[")?;

            if let Some(ref span) = self.span {
                fmt::Display::fmt(span, f)?;
            }

            let mut fields = self.fields.iter();
            if let Some(field) = fields.next() {
                write!(f, "{{{}", field)?;
                for field in fields {
                    write!(f, ",{}", field)?;
                }
                f.write_str("}")?;
            }

            f.write_str("]")?;
            wrote_any = true;
        }

        if wrote_any {
            f.write_str("=")?;
        }

        fmt::Display::fmt(&self.level, f)
    }
}
