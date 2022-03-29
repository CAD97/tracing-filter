use {
    std::{cell::RefCell, collections::HashMap, fmt, sync::RwLock},
    thread_local::ThreadLocal,
    tracing::{Collect, Metadata},
    tracing_core::{callsite, span, Interest, LevelFilter},
    tracing_subscriber::subscribe::Context,
};

mod directive;
mod matcher;
mod parse;

/// A filter matching tracing's legacy EnvFilter format.
#[derive(Debug)]
pub struct Filter {
    scope: ThreadLocal<RefCell<Vec<LevelFilter>>>,
    statics: directive::Statics,
    dynamics: directive::Dynamics,
    by_id: RwLock<HashMap<span::Id, matcher::SpanMatcher>>,
    by_cs: RwLock<HashMap<callsite::Identifier, matcher::CallsiteMatcher>>,
}

impl Filter {
    fn has_dynamics(&self) -> bool {
        !self.dynamics.directives.is_empty()
    }

    fn cares_about_span(&self, span: &span::Id) -> bool {
        let by_id = try_lock!(self.by_id.read(), else return false);
        by_id.contains_key(span)
    }

    fn base_interest(&self) -> Interest {
        if self.has_dynamics() {
            Interest::sometimes()
        } else {
            Interest::never()
        }
    }
}

impl<C: Collect> crate::Filter<C> for Filter {
    fn interest(&self, metadata: &Metadata<'_>) -> tracing_core::Interest {
        if self.has_dynamics() && metadata.is_span() {
            // If this metadata describes a span, first, check if there is a
            // dynamic filter that should be constructed for it. If so, it
            // should always be enabled, since it influences filtering.
            if let Some(matcher) = self.dynamics.matcher(metadata) {
                let mut by_cs = try_lock!(self.by_cs.write(), else return self.base_interest());
                by_cs.insert(metadata.callsite(), matcher);
                return Interest::always();
            }
        }

        if self.statics.enabled(metadata) {
            Interest::always()
        } else {
            self.base_interest()
        }
    }

    fn enabled(&self, metadata: &Metadata<'_>, _ctx: Context<'_, C>) -> bool {
        let level = metadata.level();

        // Is it possible for a dynamic filter directive to enable this event?
        // If not, we can avoid the thread local access & iterating over the
        // spans in the current scope.

        if self.has_dynamics() && self.dynamics.level >= *level {
            if metadata.is_span() {
                // If the metadata is a span, see if we care about its callsite.
                let enabled = self
                    .by_cs
                    .read()
                    .map(|cs| cs.contains_key(&metadata.callsite()))
                    .unwrap_or_default();
                if enabled {
                    return true;
                }
            }

            let enabled = self
                .scope
                .get_or_default()
                .borrow()
                .iter()
                .any(|filter| filter >= level);
            if enabled {
                return true;
            }
        }

        // Otherwise, fall back to checking if the callsite is statically enabled.
        self.statics.enabled(metadata)
    }

    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, _: Context<'_, C>) {
        let by_cs = try_lock!(self.by_cs.read());
        if let Some(cs) = by_cs.get(&attrs.metadata().callsite()) {
            let span = cs.to_span_matcher(attrs);
            try_lock!(self.by_id.write()).insert(id.clone(), span);
        }
    }

    fn on_enter(&self, id: &span::Id, _: Context<'_, C>) {
        // We _could_ push IDs to the stack instead, and use that to allow
        // changing the filter while a span is already entered. But that seems
        // much less efficient...
        if let Some(span) = try_lock!(self.by_id.read()).get(id) {
            self.scope.get_or_default().borrow_mut().push(span.level());
        }
    }

    fn on_record(&self, id: &span::Id, values: &span::Record<'_>, _ctx: Context<'_, C>) {
        if let Some(span) = try_lock!(self.by_id.read()).get(id) {
            span.record_update(values);
        }
    }

    fn on_exit(&self, id: &span::Id, _ctx: Context<'_, C>) {
        if self.cares_about_span(id) {
            self.scope.get_or_default().borrow_mut().pop();
        }
    }

    fn on_close(&self, id: &span::Id, _ctx: Context<'_, C>) {
        // If we don't need a write lock, avoid taking one.
        if !self.cares_about_span(id) {
            return;
        }

        let mut by_id = try_lock!(self.by_id.write());
        by_id.remove(id);
    }
}

impl fmt::Display for Filter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut wrote_any = false;

        for directive in self.statics.directives.iter() {
            if wrote_any {
                write!(f, ",")?;
            }
            write!(f, "{directive}")?;
            wrote_any = true;
        }

        for directive in self.dynamics.directives.iter() {
            if wrote_any {
                write!(f, ",")?;
            }
            write!(f, "{directive}")?;
            wrote_any = true;
        }

        Ok(())
    }
}
