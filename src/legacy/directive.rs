use {
    super::matcher::{CallsiteMatch, CallsiteMatcher, FieldMatch, Match},
    crate::FilterVec,
    smartstring::alias::String,
    std::collections::HashMap,
    tracing::Metadata,
    tracing_core::LevelFilter,
};

pub(super) struct DirectiveSet<T> {
    pub(super) directives: Vec<T>,
    pub(super) level: LevelFilter,
}

pub(super) type Dynamics = DirectiveSet<DynamicDirective>;
pub(super) struct DynamicDirective {
    pub(super) span: Option<String>,
    pub(super) fields: FilterVec<FieldMatch>,
    pub(super) target: Option<String>,
    pub(super) level: LevelFilter,
}

pub(super) type Statics = DirectiveSet<StaticDirective>;
pub(super) struct StaticDirective {
    pub(super) target: Option<String>,
    pub(super) fields: FilterVec<String>,
    pub(super) level: LevelFilter,
}

impl<T> DirectiveSet<T> {
    pub(super) fn directives(&self) -> impl Iterator<Item = &T> {
        self.directives.iter()
    }

    pub(super) fn directives_for<'a>(
        &'a self,
        metadata: &'a Metadata<'a>,
    ) -> impl Iterator<Item = &'a T> + 'a
    where
        T: Match,
    {
        self.directives().filter(move |d| d.cares_about(metadata))
    }
}

impl DynamicDirective {
    pub(super) fn field_matcher(&self, metadata: &Metadata<'_>) -> Option<CallsiteMatch> {
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
            Some(CallsiteMatcher { fields, level })
        } else if !fields.is_empty() {
            Some(CallsiteMatcher {
                fields,
                level: level.unwrap_or(LevelFilter::OFF),
            })
        } else {
            None
        }
    }
}
