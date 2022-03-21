use {
    crate::DEFAULT_ENV,
    miette::ErrReport,
    smartstring::alias::String as SmartString,
    std::{borrow::Cow, env, ffi::OsStr, fmt},
    tracing_core::LevelFilter,
};

mod parse;

#[derive(Debug, Default)]
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

impl Filter {
    pub const fn new() -> Self {
        Self {
            directives: Vec::new(),
            regex: None,
        }
    }

    pub fn from_env(key: impl AsRef<OsStr>) -> (Self, Option<ErrReport>) {
        if let Ok(s) = env::var(key) {
            let (filter, err) = Self::parse(s);
            (filter.unwrap_or_default(), err)
        } else {
            (Self::new(), None)
        }
    }

    pub fn from_default_env() -> (Self, Option<ErrReport>) {
        Self::from_env(DEFAULT_ENV)
    }

    pub fn add_directive<'a>(
        &mut self,
        target: Option<impl Into<Cow<'a, str>>>,
        level: impl Into<LevelFilter>,
    ) {
        let target = target.map(Into::into).map(Into::into);
        let level = level.into();
        self.directives.push(Directive { target, level });
    }

    pub fn with_directive<'a>(
        mut self,
        target: Option<impl Into<Cow<'a, str>>>,
        level: impl Into<LevelFilter>,
    ) -> Self {
        self.add_directive(target, level);
        self
    }
}

impl<C> crate::filter::Filter<C> for Filter {
    fn enabled(
        &self,
        metadata: &tracing::Metadata<'_>,
        _ctx: tracing_subscriber::subscribe::Context<'_, C>,
    ) -> bool {
        // this code is adapted directly from env_logger 0.9.0
        // env_logger is licensed under MIT OR Apache-2.0

        let level = *metadata.level();
        let target = metadata.target();

        if self.directives.is_empty() {
            return level <= LevelFilter::ERROR;
        }

        let mut enabled = false;
        for directive in &self.directives {
            match &directive.target {
                Some(name) if !target.starts_with(&**name) => {},
                Some(_) | None => enabled |= level <= directive.level,
            }
        }
        enabled
    }
}
