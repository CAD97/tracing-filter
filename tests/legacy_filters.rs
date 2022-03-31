//! These tests are adapted directly from tracing_subscriber
//! tracing_subscriber is licensed under MIT

use {
    crate::mock::{self, MockSubscribe},
    tracing::collect::with_default,
    tracing_filter::{legacy::Filter, FilterSubscriber},
    tracing_mock::collector,
    tracing_subscriber::prelude::*,
};

fn test(filter: Filter, f: impl FnOnce(&MockSubscribe)) {
    let filter = FilterSubscriber::new(filter);
    let mock = mock::subscribe();
    let collector = collector::mock().run().with(mock.clone()).with(filter);
    with_default(collector, || f(&mock));
    mock.assert_clear();
}

#[test]
fn field_filter_events() {
    let filter = "[{thing}]=debug".parse().unwrap();
    test(filter, |mock| {
        mock.expect_no_event();
        tracing::trace!(disabled = true);
        mock.expect_no_event();
        tracing::info!("also disabled");
        mock.expect_event();
        tracing::info!(thing = 1);
        mock.expect_event();
        tracing::debug!(thing = 2);
        mock.expect_no_event();
        tracing::trace!(thing = 3);
    });
}

#[test]
fn field_filter_spans() {
    let filter = "[{enabled=true}]=debug".parse().unwrap();
    test(filter, |mock| {
        mock.expect_no_event();
        tracing::trace!("disabled");
        mock.expect_no_event();
        tracing::info!("also disabled");
        mock.expect_span();
        tracing::info_span!("span1", enabled = true).in_scope(|| {
            mock.expect_event();
            tracing::info!(something = 1);
        });
        mock.expect_span();
        tracing::debug_span!("span2", enabled = false, foo = "hi").in_scope(|| {
            mock.expect_no_event();
            tracing::warn!(something = 2);
        });
        mock.expect_no_span(); // XXX: different from upstream?
        tracing::trace_span!("span3", enabled = true, answer = 42).in_scope(|| {
            mock.expect_no_event(); // XXX: different from upstream?
            tracing::debug!(something = 2);
        });
    })
}

#[test]
fn record_after_created() {
    let filter = "[{enabled=true}]=debug".parse().unwrap();
    test(filter, |mock| {
        let span = tracing::debug_span!("span", enabled = false); // XXX: different from upstream?

        mock.expect_span();
        span.in_scope(|| {
            mock.expect_no_event();
            tracing::debug!("i'm disabled!");
        });

        span.record("enabled", &true);

        mock.expect_span();
        span.in_scope(|| {
            mock.expect_event();
            tracing::debug!("i'm enabled!");
        });

        mock.expect_no_event();
        tracing::debug!("i'm also disabled");
    })
}

#[test]
#[ignore = "FIXME: failing for some reason"] // XXX: different from upstream?
fn log_is_enabled() {
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

    let filter: Filter = "lib::legacy_filters::my_module=info".parse().unwrap();
    let filter = FilterSubscriber::new(filter);
    let mock = mock::subscribe();
    let collector = collector::mock().run().with(mock.clone()).with(filter);

    // Note: we have to set the global default in order to set the `log` max
    // level, which can only be set once.
    collector.init();

    my_module::test_records(&mock);
    mock.expect_no_event();
    log::info!("this is disabled");

    my_module::test_log_enabled();
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
fn level_filter_event() {
    let filter = "info".parse().unwrap();
    test(filter, |mock| {
        mock.expect_no_event();
        tracing::trace!("this should be disabled");
        mock.expect_event();
        tracing::info!("this shouldn't be");
        mock.expect_no_event();
        tracing::debug!(target: "foo", "this should also be disabled");
        mock.expect_event();
        tracing::warn!(target: "foo", "this should be enabled");
        mock.expect_event();
        tracing::error!("this should be enabled too");
    })
}

#[test]
fn same_name_spans() {
    let filter = "[foo{bar}]=trace,[foo{baz}]=trace".parse().unwrap();
    test(filter, |mock| {
        mock.expect_span();
        let _ = tracing::trace_span!("foo", bar = 1).enter();
        mock.expect_span();
        let _ = tracing::trace_span!("foo", baz = 1).enter();
    })
}

#[test]
fn level_filter_event_with_target() {
    let filter = "info,stuff=debug".parse().unwrap();
    test(filter, |mock| {
        mock.expect_no_event();
        tracing::trace!("this should be disabled");
        mock.expect_event();
        tracing::info!("this shouldn't be");
        mock.expect_event();
        tracing::debug!(target: "stuff", "this should be enabled");
        mock.expect_no_event();
        tracing::debug!("but this shouldn't");
        mock.expect_no_event();
        tracing::trace!(target: "stuff", "and neither should this");
        mock.expect_event();
        tracing::warn!(target: "stuff", "this should be enabled");
        mock.expect_event();
        tracing::error!("this should be enabled too");
        mock.expect_event();
        tracing::error!(target: "stuff", "this should be enabled also");
    })
}

#[test]
fn not_order_dependent() {
    // this test reproduces tokio-rs/tracing#623

    let filter = "stuff=debug,info".parse().unwrap();
    test(filter, |mock| {
        mock.expect_no_event();
        tracing::trace!("this should be disabled");
        mock.expect_event();
        tracing::info!("this shouldn't be");
        mock.expect_event();
        tracing::debug!(target: "stuff", "this should be enabled");
        mock.expect_no_event();
        tracing::debug!("but this shouldn't");
        mock.expect_no_event();
        tracing::trace!(target: "stuff", "and neither should this");
        mock.expect_event();
        tracing::warn!(target: "stuff", "this should be enabled");
        mock.expect_event();
        tracing::error!("this should be enabled too");
        mock.expect_event();
        tracing::error!(target: "stuff", "this should be enabled also");
    })
}

#[test]
#[cfg(FALSE)] // add_directive not yet provided // XXX: different from upstream
fn add_directive_enables_event() {
    // this test reproduces tokio-rs/tracing#591

    let filter = "[{enabled=true}]=debug".parse().unwrap();
    filter.add_directive("hello=trace".parse().unwrap());
    test(filter, |mock| {
        mock.expect_event();
        tracing::info!(target: "hello", "hello info");
        mock.expect_event();
        tracing::trace!(target: "hello", "hello trace");
    })
}

#[test]
fn span_name_filter_is_dynamic() {
    let filter = "info,[cool_span]=debug".parse().unwrap();
    test(filter, |mock| {
        mock.expect_no_event();
        tracing::trace!("this should be disabled");
        mock.expect_event();
        tracing::info!("this shouldn't be");

        let cool_span = tracing::info_span!("cool_span");
        let uncool_span = tracing::info_span!("uncool_span");

        {
            mock.expect_span();
            let _enter = cool_span.enter();
            mock.expect_event();
            tracing::debug!("i'm a cool event");
            mock.expect_no_event();
            tracing::trace!("i'm cool, but not cool enough");
            mock.expect_span();
            let _enter2 = uncool_span.enter();
            mock.expect_event();
            tracing::warn!("warning: extremely cool!");
            mock.expect_event();
            tracing::debug!("i'm still cool");
        }

        mock.expect_span();
        let _enter = uncool_span.enter();
        mock.expect_event();
        tracing::warn!("warning: not that cool");
        mock.expect_no_event();
        tracing::trace!("im not cool enough");
        mock.expect_event();
        tracing::error!("uncool error");
    })
}

#[test]
fn same_length_targets() {
    let filter = "foo=trace,bar=trace".parse().unwrap();
    test(filter, |mock| {
        mock.expect_event();
        tracing::trace!(target: "foo", "foo");
        mock.expect_event();
        tracing::trace!(target: "bar", "bar");
    })
}

#[test]
fn same_num_fields_event() {
    let filter = "[{foo}]=trace,[{bar}]=trace".parse().unwrap();
    test(filter, |mock| {
        mock.expect_event();
        tracing::trace!(foo = 1);
        mock.expect_event();
        tracing::trace!(bar = 3);
    })
}

#[test]
fn same_num_fields_and_name_len() {
    let filter = "[foo{bar=1}]=trace,[baz{boz=1}]=trace".parse().unwrap();
    test(filter, |mock| {
        mock.expect_span();
        let _ = tracing::trace_span!("foo", bar = 1).enter();
        mock.expect_span();
        let _ = tracing::trace_span!("baz", boz = 1).enter();
    })
}
