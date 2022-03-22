//! These tests are adapted directly from env_logger 0.9.0
//! env_logger is licensed under MIT OR Apache-2.0

use tracing::{collect::with_default, level_filters::LevelFilter};
use tracing_filter::{simple::Filter, FilterSubscriber};
use tracing_mock::*;
use tracing_subscriber::{subscribe::CollectExt, Registry};

fn test(filter: Filter, f: impl FnOnce(&MockSubscribe)) {
    let filter = FilterSubscriber::new(filter);
    let mock = subscribe::mock().strict(subscribe::ExpectKind::EVENT);
    let collector = Registry::default().with(mock.clone()).with(filter);
    with_default(collector, || f(&mock));
    mock.finish();
}

#[test]
fn filter_info() {
    let filter = Filter::new().with_level(LevelFilter::INFO);
    test(filter, |mock| {
        mock.expect(expect![Event]);
        tracing::info!(target: "crate1", "");
        mock.expect(expect![]);
        tracing::debug!(target: "crate1", "");
    });
}

#[test]
fn filter_beginning_longest_match() {
    let filter = Filter::new()
        .with_target("crate2", LevelFilter::INFO)
        .with_target("crate2::mod", LevelFilter::DEBUG)
        .with_target("crate1::mod1", LevelFilter::WARN);
    test(filter, |mock| {
        mock.expect(expect![Event]);
        tracing::debug!(target: "crate2::mod1", "");
        mock.expect(expect![]);
        tracing::debug!(target: "crate2", "");
    });
}

#[test]
fn parse_default() {
    let filter = "info,crate1::mod1=warn".parse().unwrap();
    test(filter, |mock| {
        mock.expect(expect![Event]);
        tracing::warn!(target: "crate1::mod1", "");
        mock.expect(expect![Event]);
        tracing::info!(target: "crate2::mod2", "");
    });
}

#[test]
fn parse_default_bare_level_off_lc() {
    let filter = "off".parse().unwrap();
    test(filter, |mock| {
        mock.expect(expect![]);
        tracing::error!(target: "", "");
        mock.expect(expect![]);
        tracing::warn!(target: "", "");
        mock.expect(expect![]);
        tracing::info!(target: "", "");
        mock.expect(expect![]);
        tracing::debug!(target: "", "");
        mock.expect(expect![]);
        tracing::trace!(target: "", "");
    });
}

#[test]
fn parse_default_bare_level_off_uc() {
    let filter = "OFF".parse().unwrap();
    test(filter, |mock| {
        mock.expect(expect![]);
        tracing::error!(target: "", "");
        mock.expect(expect![]);
        tracing::warn!(target: "", "");
        mock.expect(expect![]);
        tracing::info!(target: "", "");
        mock.expect(expect![]);
        tracing::debug!(target: "", "");
        mock.expect(expect![]);
        tracing::trace!(target: "", "");
    });
}

#[test]
fn parse_default_bare_level_error_lc() {
    let filter = "error".parse().unwrap();
    test(filter, |mock| {
        mock.expect(expect![Event]);
        tracing::error!(target: "", "");
        mock.expect(expect![]);
        tracing::warn!(target: "", "");
        mock.expect(expect![]);
        tracing::info!(target: "", "");
        mock.expect(expect![]);
        tracing::debug!(target: "", "");
        mock.expect(expect![]);
        tracing::trace!(target: "", "");
    });
}

#[test]
fn parse_default_bare_level_error_uc() {
    let filter = "ERROR".parse().unwrap();
    test(filter, |mock| {
        mock.expect(expect![Event]);
        tracing::error!(target: "", "");
        mock.expect(expect![]);
        tracing::warn!(target: "", "");
        mock.expect(expect![]);
        tracing::info!(target: "", "");
        mock.expect(expect![]);
        tracing::debug!(target: "", "");
        mock.expect(expect![]);
        tracing::trace!(target: "", "");
    });
}

#[test]
fn parse_default_bare_level_warn_lc() {
    let filter = "warn".parse().unwrap();
    test(filter, |mock| {
        mock.expect(expect![Event]);
        tracing::error!(target: "", "");
        mock.expect(expect![Event]);
        tracing::warn!(target: "", "");
        mock.expect(expect![]);
        tracing::info!(target: "", "");
        mock.expect(expect![]);
        tracing::debug!(target: "", "");
        mock.expect(expect![]);
        tracing::trace!(target: "", "");
    });
}

#[test]
fn parse_default_bare_level_warn_uc() {
    let filter = "WARN".parse().unwrap();
    test(filter, |mock| {
        mock.expect(expect![Event]);
        tracing::error!(target: "", "");
        mock.expect(expect![Event]);
        tracing::warn!(target: "", "");
        mock.expect(expect![]);
        tracing::info!(target: "", "");
        mock.expect(expect![]);
        tracing::debug!(target: "", "");
        mock.expect(expect![]);
        tracing::trace!(target: "", "");
    });
}

#[test]
fn parse_default_bare_level_info_lc() {
    let filter = "info".parse().unwrap();
    test(filter, |mock| {
        mock.expect(expect![Event]);
        tracing::error!(target: "", "");
        mock.expect(expect![Event]);
        tracing::warn!(target: "", "");
        mock.expect(expect![Event]);
        tracing::info!(target: "", "");
        mock.expect(expect![]);
        tracing::debug!(target: "", "");
        mock.expect(expect![]);
        tracing::trace!(target: "", "");
    });
}

#[test]
fn parse_default_bare_level_info_uc() {
    let filter = "INFO".parse().unwrap();
    test(filter, |mock| {
        mock.expect(expect![Event]);
        tracing::error!(target: "", "");
        mock.expect(expect![Event]);
        tracing::warn!(target: "", "");
        mock.expect(expect![Event]);
        tracing::info!(target: "", "");
        mock.expect(expect![]);
        tracing::debug!(target: "", "");
        mock.expect(expect![]);
        tracing::trace!(target: "", "");
    });
}

#[test]
fn parse_default_bare_level_debug_lc() {
    let filter = "debug".parse().unwrap();
    test(filter, |mock| {
        mock.expect(expect![Event]);
        tracing::error!(target: "", "");
        mock.expect(expect![Event]);
        tracing::warn!(target: "", "");
        mock.expect(expect![Event]);
        tracing::info!(target: "", "");
        mock.expect(expect![Event]);
        tracing::debug!(target: "", "");
        mock.expect(expect![]);
        tracing::trace!(target: "", "");
    });
}

#[test]
fn parse_default_bare_level_debug_uc() {
    let filter = "DEBUG".parse().unwrap();
    test(filter, |mock| {
        mock.expect(expect![Event]);
        tracing::error!(target: "", "");
        mock.expect(expect![Event]);
        tracing::warn!(target: "", "");
        mock.expect(expect![Event]);
        tracing::info!(target: "", "");
        mock.expect(expect![Event]);
        tracing::debug!(target: "", "");
        mock.expect(expect![]);
        tracing::trace!(target: "", "");
    });
}

#[test]
fn parse_default_bare_level_trace_lc() {
    let filter = "trace".parse().unwrap();
    test(filter, |mock| {
        mock.expect(expect![Event]);
        tracing::error!(target: "", "");
        mock.expect(expect![Event]);
        tracing::warn!(target: "", "");
        mock.expect(expect![Event]);
        tracing::info!(target: "", "");
        mock.expect(expect![Event]);
        tracing::debug!(target: "", "");
        mock.expect(expect![Event]);
        tracing::trace!(target: "", "");
    });
}

#[test]
fn parse_default_bare_level_trace_uc() {
    let filter = "TRACE".parse().unwrap();
    test(filter, |mock| {
        mock.expect(expect![Event]);
        tracing::error!(target: "", "");
        mock.expect(expect![Event]);
        tracing::warn!(target: "", "");
        mock.expect(expect![Event]);
        tracing::info!(target: "", "");
        mock.expect(expect![Event]);
        tracing::debug!(target: "", "");
        mock.expect(expect![Event]);
        tracing::trace!(target: "", "");
    });
}

// In practice, the desired log level is typically specified by a token
// that is either all lowercase (e.g., 'trace') or all uppercase (.e.g,
// 'TRACE'), but this tests serves as a reminder that
// log::Level::from_str() ignores all case variants.
#[test]
fn parse_default_bare_level_debug_mixed() {
    let filter = "Debug".parse().unwrap();
    test(filter, |mock| {
        mock.expect(expect![Event]);
        tracing::error!(target: "", "");
        mock.expect(expect![Event]);
        tracing::warn!(target: "", "");
        mock.expect(expect![Event]);
        tracing::info!(target: "", "");
        mock.expect(expect![Event]);
        tracing::debug!(target: "", "");
        mock.expect(expect![]);
        tracing::trace!(target: "", "");
    });

    let filter = "debuG".parse().unwrap();
    test(filter, |mock| {
        mock.expect(expect![Event]);
        tracing::error!(target: "", "");
        mock.expect(expect![Event]);
        tracing::warn!(target: "", "");
        mock.expect(expect![Event]);
        tracing::info!(target: "", "");
        mock.expect(expect![Event]);
        tracing::debug!(target: "", "");
        mock.expect(expect![]);
        tracing::trace!(target: "", "");
    });

    let filter = "deBug".parse().unwrap();
    test(filter, |mock| {
        mock.expect(expect![Event]);
        tracing::error!(target: "", "");
        mock.expect(expect![Event]);
        tracing::warn!(target: "", "");
        mock.expect(expect![Event]);
        tracing::info!(target: "", "");
        mock.expect(expect![Event]);
        tracing::debug!(target: "", "");
        mock.expect(expect![]);
        tracing::trace!(target: "", "");
    });

    let filter = "DeBuG".parse().unwrap(); // LaTeX flavor!
    test(filter, |mock| {
        mock.expect(expect![Event]);
        tracing::error!(target: "", "");
        mock.expect(expect![Event]);
        tracing::warn!(target: "", "");
        mock.expect(expect![Event]);
        tracing::info!(target: "", "");
        mock.expect(expect![Event]);
        tracing::debug!(target: "", "");
        mock.expect(expect![]);
        tracing::trace!(target: "", "");
    });
}

#[test]
fn match_full_path() {
    let filter = Filter::new()
        .with_target("crate2", LevelFilter::INFO)
        .with_target("crate1::mod1", LevelFilter::WARN);
    test(filter, |mock| {
        mock.expect(expect![Event]);
        tracing::warn!(target: "crate1::mod1", "");
        mock.expect(expect![]);
        tracing::info!(target: "crate1::mod1", "");
        mock.expect(expect![Event]);
        tracing::info!(target: "crate2", "");
        mock.expect(expect![]);
        tracing::debug!(target: "crate2", "");
    })
}

#[test]
fn no_match() {
    let filter = Filter::new()
        .with_target("crate2", LevelFilter::INFO)
        .with_target("crate1::mod1", LevelFilter::WARN);
    test(filter, |mock| {
        mock.expect(expect![]);
        tracing::warn!(target: "crate3", "");
    });
}

#[test]
fn match_beginning() {
    let filter = Filter::new()
        .with_target("crate2", LevelFilter::INFO)
        .with_target("crate1::mod1", LevelFilter::WARN);
    test(filter, |mock| {
        mock.expect(expect![Event]);
        tracing::warn!(target: "crate2::mod1", "");
    });
}

#[test]
fn match_beginning_longest_match() {
    let filter = Filter::new()
        .with_target("crate2", LevelFilter::INFO)
        .with_target("crate2::mod", LevelFilter::DEBUG)
        .with_target("crate1::mod1", LevelFilter::WARN);
    test(filter, |mock| {
        mock.expect(expect![Event]);
        tracing::debug!(target: "crate2::mod1", "");
        mock.expect([]);
        tracing::debug!(target: "crate2", "");
    })
}

#[test]
fn match_default() {
    let filter = Filter::new()
        .with_level(LevelFilter::INFO)
        .with_target("crate1::mod1", LevelFilter::WARN);
    test(filter, |mock| {
        mock.expect(expect![Event]);
        tracing::warn!(target: "crate1::mod1", "");
        mock.expect(expect![Event]);
        tracing::warn!(target: "crate2::mod2", "");
    });
}

#[test]
fn zero_level() {
    let filter = Filter::new()
        .with_level(LevelFilter::INFO)
        .with_target("crate1::mod1", LevelFilter::OFF);
    test(filter, |mock| {
        mock.expect(expect![]);
        tracing::error!(target: "crate1::mod1", "");
        mock.expect(expect![Event]);
        tracing::info!(target: "crate2::mod2", "");
    })
}

// parse_spec_* tests are in test_cases
