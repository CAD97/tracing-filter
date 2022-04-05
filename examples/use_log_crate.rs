//! This tests is adapted directly from tracing_subscriber
//! tracing_subscriber is licensed under MIT

// I'm sorry rust-analzyer
#[path = "../tests/mock.rs"]
#[allow(unused)]
mod mock;

use {
    crate::mock::MockSubscribe,
    tracing::Level,
    tracing_filter::{legacy::Filter, FilterSubscriber},
    tracing_subscriber::prelude::*,
};

mod my_module {
    use super::*;
    pub(super) fn test_records(mock: &MockSubscribe) {
        mock.expect_no_event();
        dbg!(module_path!());
        log::trace!("this should be disabled");
        mock.expect_event();
        log::info!("this shouldn't be");
        mock.expect_no_event();
        log::debug!("this should be disabled");
        mock.expect_event();
        log::warn!("this should be enabled");
        mock.expect_no_event();
        log::warn!(target: "something else", "this shouldn't be enabled");
        mock.expect_event();
        log::error!("this should be enabled too");
    }

    pub(super) fn test_log_enabled() {
        assert!(
            log::log_enabled!(log::Level::Info),
            "info should be enabled inside `my_module`"
        );
        assert!(
            !log::log_enabled!(log::Level::Debug),
            "debug should not be enabled inside `my_module`"
        );
        assert!(
            log::log_enabled!(log::Level::Warn),
            "warn should be enabled inside `my_module`"
        );
    }
}

fn main() {
    let filter: Filter = "use_log_crate::my_module=info".parse().unwrap();
    let filter = FilterSubscriber::new(filter);
    let mock = mock::subscribe();
    let collector = tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .finish()
        .with(mock.clone())
        .with(filter);

    // Note: we have to set the global default in order to set the `log` max
    // level, which can only be set once.
    collector.init();

    my_module::test_log_enabled();
    my_module::test_records(&mock);

    mock.expect_no_event();
    log::info!("this is disabled");

    assert!(
        !log::log_enabled!(log::Level::Info),
        "info should not be enabled outside `my_module`"
    );
    assert!(
        !log::log_enabled!(log::Level::Warn),
        "warn should not be enabled outside `my_module`"
    );

    mock.assert_clear();
}

#[test]
fn test_it() {
    main()
}
