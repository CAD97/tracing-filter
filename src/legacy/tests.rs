use {
    super::{directive::*, *},
    crate::FilterSubscriber,
    tracing_core::{field::FieldSet, *},
    tracing_subscriber::prelude::*,
};

struct NoCollector;
impl Collect for NoCollector {
    #[inline]
    fn register_callsite(&self, _: &'static Metadata<'static>) -> collect::Interest {
        collect::Interest::always()
    }
    fn new_span(&self, _: &span::Attributes<'_>) -> span::Id {
        span::Id::from_u64(0xDEAD)
    }
    fn event(&self, _event: &Event<'_>) {}
    fn record(&self, _span: &span::Id, _values: &span::Record<'_>) {}
    fn record_follows_from(&self, _span: &span::Id, _follows: &span::Id) {}

    #[inline]
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }
    fn enter(&self, _span: &span::Id) {}
    fn exit(&self, _span: &span::Id) {}
    fn current_span(&self) -> span::Current {
        span::Current::unknown()
    }
}

struct Cs;
impl Callsite for Cs {
    fn set_interest(&self, _interest: Interest) {}
    fn metadata(&self) -> &Metadata<'_> {
        unimplemented!()
    }
}

#[test]
fn callsite_enabled_no_span_directive() {
    let filter = FilterSubscriber::new(Filter::new("app=debug")).with_collector(NoCollector);
    static META: &Metadata<'static> = &Metadata::new(
        "mySpan",
        "app",
        Level::TRACE,
        None,
        None,
        None,
        FieldSet::new(&[], identify_callsite!(&Cs)),
        Kind::SPAN,
    );

    let interest = filter.register_callsite(META);
    assert!(interest.is_never());
}

#[test]
fn callsite_off() {
    let filter = FilterSubscriber::new(Filter::new("app=off")).with_collector(NoCollector);
    static META: &Metadata<'static> = &Metadata::new(
        "mySpan",
        "app",
        Level::ERROR,
        None,
        None,
        None,
        FieldSet::new(&[], identify_callsite!(&Cs)),
        Kind::SPAN,
    );

    let interest = filter.register_callsite(META);
    assert!(interest.is_never());
}

#[test]
fn callsite_enabled_includes_span_directive() {
    let filter =
        FilterSubscriber::new(Filter::new("app[mySpan]=debug")).with_collector(NoCollector);
    static META: &Metadata<'static> = &Metadata::new(
        "mySpan",
        "app",
        Level::TRACE,
        None,
        None,
        None,
        FieldSet::new(&[], identify_callsite!(&Cs)),
        Kind::SPAN,
    );

    let interest = filter.register_callsite(META);
    assert!(interest.is_always());
}

#[test]
fn callsite_enabled_includes_span_directive_field() {
    let filter = FilterSubscriber::new(Filter::new("app[mySpan{field=\"value\"}]=debug"))
        .with_collector(NoCollector);
    static META: &Metadata<'static> = &Metadata::new(
        "mySpan",
        "app",
        Level::TRACE,
        None,
        None,
        None,
        FieldSet::new(&["field"], identify_callsite!(&Cs)),
        Kind::SPAN,
    );

    let interest = filter.register_callsite(META);
    assert!(interest.is_always());
}

#[test]
#[ignore = "filter parser doesn't support multiple fields"]
fn callsite_enabled_includes_span_directive_multiple_fields() {
    let filter = FilterSubscriber::new(
        "app[mySpan{field=\"value\",field2=2}]=debug"
            .parse::<Filter>()
            .expect("filter should parse without warnings"),
    )
    .with_collector(NoCollector);
    static META: &Metadata<'static> = &Metadata::new(
        "mySpan",
        "app",
        Level::TRACE,
        None,
        None,
        None,
        FieldSet::new(&["field"], identify_callsite!(&Cs)),
        Kind::SPAN,
    );

    let interest = filter.register_callsite(META);
    assert!(interest.is_never());
}

#[test]
fn roundtrip() {
    let f1: Filter = "[span1{foo=1}]=error,[span2{bar=2 baz=false}],crate2[{quux=\"quuux\"}]=debug"
        .parse()
        .unwrap();
    let f2: Filter = format!("{}", f1).parse().unwrap();
    assert_eq!(f1.statics, f2.statics);
    assert_eq!(f1.dynamics, f2.dynamics);
}

#[test]
fn size_of_filters() {
    fn assert_sz(s: &str) {
        let filter = s.parse::<Filter>().expect("filter should parse");
        #[cfg(target_pointer_width = "64")]
        assert_eq!(
            std::mem::size_of_val(&filter),
            92 * std::mem::size_of::<usize>()
        );
        #[cfg(target_pointer_width = "32")]
        assert_eq!(
            std::mem::size_of_val(&filter),
            64 * std::mem::size_of::<usize>()
        );
        #[cfg(target_pointer_width = "16")]
        panic!("adventurous, aren't you; I'm surprised you even got this far")
    }

    assert_sz("info");

    assert_sz("foo=debug");

    assert_sz(
        "crate1::mod1=error,crate1::mod2=warn,crate1::mod2::mod3=info,\
        crate2=debug,crate3=trace,crate3::mod2::mod1=off",
    );

    assert_sz("[span1{foo=1}]=error,[span2{bar=2 baz=false}],crate2[{quux=\"quuux\"}]=debug");

    assert_sz(
        "crate1::mod1=error,crate1::mod2=warn,crate1::mod2::mod3=info,\
        crate2=debug,crate3=trace,crate3::mod2::mod1=off,[span1{foo=1}]=error,\
        [span2{bar=2 baz=false}],crate2[{quux=\"quuux\"}]=debug",
    );
}

fn parse_directives(dirs: impl AsRef<str>) -> Vec<DynamicDirective> {
    dirs.as_ref()
        .split(',')
        .filter_map(|s| s.parse().ok())
        .collect()
}

fn expect_parse(dirs: impl AsRef<str>) -> Vec<DynamicDirective> {
    dirs.as_ref()
        .split(',')
        .map(|s| {
            s.parse()
                .unwrap_or_else(|err| panic!("directive '{:?}' should parse: {}", s, err))
        })
        .collect()
}

#[test]
fn directive_ordering_by_target_len() {
    // TODO(eliza): it would be nice to have a property-based test for this
    // instead.
    let mut dirs = expect_parse(
        "foo::bar=debug,foo::bar::baz=trace,foo=info,a_really_long_name_with_no_colons=warn",
    );
    dirs.sort_unstable();

    let expected = vec![
        "a_really_long_name_with_no_colons",
        "foo::bar::baz",
        "foo::bar",
        "foo",
    ];
    let sorted = dirs
        .iter()
        .map(|d| d.target.as_deref().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(expected, sorted);
}
#[test]
fn directive_ordering_by_span() {
    // TODO(eliza): it would be nice to have a property-based test for this
    // instead.
    let mut dirs = expect_parse("bar[span]=trace,foo=debug,baz::quux=info,a[span]=warn");
    dirs.sort_unstable();

    let expected = vec!["baz::quux", "bar", "foo", "a"];
    let sorted = dirs
        .iter()
        .map(|d| d.target.as_deref().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(expected, sorted);
}

#[test]
fn directive_ordering_uses_lexicographic_when_equal() {
    // TODO(eliza): it would be nice to have a property-based test for this
    // instead.
    let mut dirs = expect_parse("span[b]=debug,b=debug,a=trace,c=info,span[a]=info");
    dirs.sort_unstable();

    let expected = vec![
        ("span", Some("b")),
        ("span", Some("a")),
        ("c", None),
        ("b", None),
        ("a", None),
    ];
    let sorted = dirs
        .iter()
        .map(|d| {
            (
                d.target.as_ref().unwrap().as_ref(),
                d.span.as_ref().map(AsRef::as_ref),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(expected, sorted);
}

// TODO: this test requires the parser to support directives with multiple
// fields, which it currently can't handle. We should enable this test when
// that's implemented.
#[test]
#[ignore = "filter parser doesn't support multiple fields"]
fn directive_ordering_by_field_num() {
    // TODO(eliza): it would be nice to have a property-based test for this
    // instead.
    let mut dirs = expect_parse(
        "b[{foo,bar}]=info,c[{baz,quuux,quuux}]=debug,a[{foo}]=warn,bar[{field}]=trace,foo=debug,baz::quux=info"
    );
    dirs.sort_unstable();

    let expected = vec!["baz::quux", "bar", "foo", "c", "b", "a"];
    let sorted = dirs
        .iter()
        .map(|d| d.target.as_deref().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(expected, sorted);
}

#[test]
fn parse_directives_ralith() {
    let dirs = parse_directives("common=trace,server=trace");
    assert_eq!(dirs.len(), 2, "\nparsed: {:#?}", dirs);
    assert_eq!(dirs[0].target, Some("common".into()));
    assert_eq!(dirs[0].level, LevelFilter::TRACE);
    assert_eq!(dirs[0].span, None);

    assert_eq!(dirs[1].target, Some("server".into()));
    assert_eq!(dirs[1].level, LevelFilter::TRACE);
    assert_eq!(dirs[1].span, None);
}

#[test]
fn parse_directives_ralith_uc() {
    let dirs = parse_directives("common=INFO,server=DEBUG");
    assert_eq!(dirs.len(), 2, "\nparsed: {:#?}", dirs);
    assert_eq!(dirs[0].target, Some("common".into()));
    assert_eq!(dirs[0].level, LevelFilter::INFO);
    assert_eq!(dirs[0].span, None);

    assert_eq!(dirs[1].target, Some("server".into()));
    assert_eq!(dirs[1].level, LevelFilter::DEBUG);
    assert_eq!(dirs[1].span, None);
}

#[test]
fn parse_directives_ralith_mixed() {
    let dirs = parse_directives("common=iNfo,server=dEbUg");
    assert_eq!(dirs.len(), 2, "\nparsed: {:#?}", dirs);
    assert_eq!(dirs[0].target, Some("common".into()));
    assert_eq!(dirs[0].level, LevelFilter::INFO);
    assert_eq!(dirs[0].span, None);

    assert_eq!(dirs[1].target, Some("server".into()));
    assert_eq!(dirs[1].level, LevelFilter::DEBUG);
    assert_eq!(dirs[1].span, None);
}

#[test]
fn parse_directives_valid() {
    let dirs = parse_directives("crate1::mod1=error,crate1::mod2,crate2=debug,crate3=off");
    assert_eq!(dirs.len(), 4, "\nparsed: {:#?}", dirs);
    assert_eq!(dirs[0].target, Some("crate1::mod1".into()));
    assert_eq!(dirs[0].level, LevelFilter::ERROR);
    assert_eq!(dirs[0].span, None);

    assert_eq!(dirs[1].target, Some("crate1::mod2".into()));
    assert_eq!(dirs[1].level, LevelFilter::TRACE);
    assert_eq!(dirs[1].span, None);

    assert_eq!(dirs[2].target, Some("crate2".into()));
    assert_eq!(dirs[2].level, LevelFilter::DEBUG);
    assert_eq!(dirs[2].span, None);

    assert_eq!(dirs[3].target, Some("crate3".into()));
    assert_eq!(dirs[3].level, LevelFilter::OFF);
    assert_eq!(dirs[3].span, None);
}

#[test]

fn parse_level_directives() {
    let dirs = parse_directives(
        "crate1::mod1=error,crate1::mod2=warn,crate1::mod2::mod3=info,\
         crate2=debug,crate3=trace,crate3::mod2::mod1=off",
    );
    assert_eq!(dirs.len(), 6, "\nparsed: {:#?}", dirs);
    assert_eq!(dirs[0].target, Some("crate1::mod1".into()));
    assert_eq!(dirs[0].level, LevelFilter::ERROR);
    assert_eq!(dirs[0].span, None);

    assert_eq!(dirs[1].target, Some("crate1::mod2".into()));
    assert_eq!(dirs[1].level, LevelFilter::WARN);
    assert_eq!(dirs[1].span, None);

    assert_eq!(dirs[2].target, Some("crate1::mod2::mod3".into()));
    assert_eq!(dirs[2].level, LevelFilter::INFO);
    assert_eq!(dirs[2].span, None);

    assert_eq!(dirs[3].target, Some("crate2".into()));
    assert_eq!(dirs[3].level, LevelFilter::DEBUG);
    assert_eq!(dirs[3].span, None);

    assert_eq!(dirs[4].target, Some("crate3".into()));
    assert_eq!(dirs[4].level, LevelFilter::TRACE);
    assert_eq!(dirs[4].span, None);

    assert_eq!(dirs[5].target, Some("crate3::mod2::mod1".into()));
    assert_eq!(dirs[5].level, LevelFilter::OFF);
    assert_eq!(dirs[5].span, None);
}

#[test]
fn parse_uppercase_level_directives() {
    let dirs = parse_directives(
        "crate1::mod1=ERROR,crate1::mod2=WARN,crate1::mod2::mod3=INFO,\
         crate2=DEBUG,crate3=TRACE,crate3::mod2::mod1=OFF",
    );
    assert_eq!(dirs.len(), 6, "\nparsed: {:#?}", dirs);
    assert_eq!(dirs[0].target, Some("crate1::mod1".into()));
    assert_eq!(dirs[0].level, LevelFilter::ERROR);
    assert_eq!(dirs[0].span, None);

    assert_eq!(dirs[1].target, Some("crate1::mod2".into()));
    assert_eq!(dirs[1].level, LevelFilter::WARN);
    assert_eq!(dirs[1].span, None);

    assert_eq!(dirs[2].target, Some("crate1::mod2::mod3".into()));
    assert_eq!(dirs[2].level, LevelFilter::INFO);
    assert_eq!(dirs[2].span, None);

    assert_eq!(dirs[3].target, Some("crate2".into()));
    assert_eq!(dirs[3].level, LevelFilter::DEBUG);
    assert_eq!(dirs[3].span, None);

    assert_eq!(dirs[4].target, Some("crate3".into()));
    assert_eq!(dirs[4].level, LevelFilter::TRACE);
    assert_eq!(dirs[4].span, None);

    assert_eq!(dirs[5].target, Some("crate3::mod2::mod1".into()));
    assert_eq!(dirs[5].level, LevelFilter::OFF);
    assert_eq!(dirs[5].span, None);
}

#[test]
fn parse_numeric_level_directives() {
    let dirs = parse_directives(
        "crate1::mod1=1,crate1::mod2=2,crate1::mod2::mod3=3,crate2=4,\
         crate3=5,crate3::mod2::mod1=0",
    );
    assert_eq!(dirs.len(), 6, "\nparsed: {:#?}", dirs);
    assert_eq!(dirs[0].target, Some("crate1::mod1".into()));
    assert_eq!(dirs[0].level, LevelFilter::ERROR);
    assert_eq!(dirs[0].span, None);

    assert_eq!(dirs[1].target, Some("crate1::mod2".into()));
    assert_eq!(dirs[1].level, LevelFilter::WARN);
    assert_eq!(dirs[1].span, None);

    assert_eq!(dirs[2].target, Some("crate1::mod2::mod3".into()));
    assert_eq!(dirs[2].level, LevelFilter::INFO);
    assert_eq!(dirs[2].span, None);

    assert_eq!(dirs[3].target, Some("crate2".into()));
    assert_eq!(dirs[3].level, LevelFilter::DEBUG);
    assert_eq!(dirs[3].span, None);

    assert_eq!(dirs[4].target, Some("crate3".into()));
    assert_eq!(dirs[4].level, LevelFilter::TRACE);
    assert_eq!(dirs[4].span, None);

    assert_eq!(dirs[5].target, Some("crate3::mod2::mod1".into()));
    assert_eq!(dirs[5].level, LevelFilter::OFF);
    assert_eq!(dirs[5].span, None);
}

#[test]
fn parse_directives_invalid_crate() {
    // test parse_directives with multiple = in specification
    let dirs = parse_directives("crate1::mod1=warn=info,crate2=debug");
    assert_eq!(dirs.len(), 1, "\nparsed: {:#?}", dirs);
    assert_eq!(dirs[0].target, Some("crate2".into()));
    assert_eq!(dirs[0].level, LevelFilter::DEBUG);
    assert_eq!(dirs[0].span, None);
}

#[test]
fn parse_directives_invalid_level() {
    // test parse_directives with 'noNumber' as log level
    let dirs = parse_directives("crate1::mod1=noNumber,crate2=debug");
    assert_eq!(dirs.len(), 1, "\nparsed: {:#?}", dirs);
    assert_eq!(dirs[0].target, Some("crate2".into()));
    assert_eq!(dirs[0].level, LevelFilter::DEBUG);
    assert_eq!(dirs[0].span, None);
}

#[test]
fn parse_directives_string_level() {
    // test parse_directives with 'warn' as log level
    let dirs = parse_directives("crate1::mod1=wrong,crate2=warn");
    assert_eq!(dirs.len(), 1, "\nparsed: {:#?}", dirs);
    assert_eq!(dirs[0].target, Some("crate2".into()));
    assert_eq!(dirs[0].level, LevelFilter::WARN);
    assert_eq!(dirs[0].span, None);
}

#[test]
fn parse_directives_empty_level() {
    // test parse_directives with '' as log level
    let dirs = parse_directives("crate1::mod1=wrong,crate2=");
    assert_eq!(dirs.len(), 1, "\nparsed: {:#?}", dirs);
    assert_eq!(dirs[0].target, Some("crate2".into()));
    assert_eq!(dirs[0].level, LevelFilter::TRACE);
    assert_eq!(dirs[0].span, None);
}

#[test]
fn parse_directives_global() {
    // test parse_directives with no crate
    let dirs = parse_directives("warn,crate2=debug");
    assert_eq!(dirs.len(), 2, "\nparsed: {:#?}", dirs);
    assert_eq!(dirs[0].target, None);
    assert_eq!(dirs[0].level, LevelFilter::WARN);
    assert_eq!(dirs[1].span, None);

    assert_eq!(dirs[1].target, Some("crate2".into()));
    assert_eq!(dirs[1].level, LevelFilter::DEBUG);
    assert_eq!(dirs[1].span, None);
}

// helper function for tests below
fn test_parse_bare_level(directive_to_test: &str, level_expected: LevelFilter) {
    let dirs = parse_directives(directive_to_test);
    assert_eq!(
        dirs.len(),
        1,
        "\ninput: \"{}\"; parsed: {:#?}",
        directive_to_test,
        dirs
    );
    assert_eq!(dirs[0].target, None);
    assert_eq!(dirs[0].level, level_expected);
    assert_eq!(dirs[0].span, None);
}

#[test]
fn parse_directives_global_bare_warn_lc() {
    // test parse_directives with no crate, in isolation, all lowercase
    test_parse_bare_level("warn", LevelFilter::WARN);
}

#[test]
fn parse_directives_global_bare_warn_uc() {
    // test parse_directives with no crate, in isolation, all uppercase
    test_parse_bare_level("WARN", LevelFilter::WARN);
}

#[test]
fn parse_directives_global_bare_warn_mixed() {
    // test parse_directives with no crate, in isolation, mixed case
    test_parse_bare_level("wArN", LevelFilter::WARN);
}

#[test]
fn parse_directives_valid_with_spans() {
    let dirs = parse_directives("crate1::mod1[foo]=error,crate1::mod2[bar],crate2[baz]=debug");
    assert_eq!(dirs.len(), 3, "\nparsed: {:#?}", dirs);
    assert_eq!(dirs[0].target, Some("crate1::mod1".into()));
    assert_eq!(dirs[0].level, LevelFilter::ERROR);
    assert_eq!(dirs[0].span, Some("foo".into()));

    assert_eq!(dirs[1].target, Some("crate1::mod2".into()));
    assert_eq!(dirs[1].level, LevelFilter::TRACE);
    assert_eq!(dirs[1].span, Some("bar".into()));

    assert_eq!(dirs[2].target, Some("crate2".into()));
    assert_eq!(dirs[2].level, LevelFilter::DEBUG);
    assert_eq!(dirs[2].span, Some("baz".into()));
}

#[test]
fn parse_directives_with_dash_in_target_name() {
    let dirs = parse_directives("target-name=info");
    assert_eq!(dirs.len(), 1, "\nparsed: {:#?}", dirs);
    assert_eq!(dirs[0].target, Some("target-name".into()));
    assert_eq!(dirs[0].level, LevelFilter::INFO);
    assert_eq!(dirs[0].span, None);
}

#[test]
fn parse_directives_with_dash_in_span_name() {
    // Reproduces https://github.com/tokio-rs/tracing/issues/1367

    let dirs = parse_directives("target[span-name]=info");
    assert_eq!(dirs.len(), 1, "\nparsed: {:#?}", dirs);
    assert_eq!(dirs[0].target, Some("target".into()));
    assert_eq!(dirs[0].level, LevelFilter::INFO);
    assert_eq!(dirs[0].span, Some("span-name".into()));
}

#[test]
fn parse_directives_with_special_characters_in_span_name() {
    let span_name = "!\"#$%&'()*+-./:;<=>?@^_`|~[}";

    let dirs = parse_directives(format!("target[{}]=info", span_name));
    assert_eq!(dirs.len(), 1, "\nparsed: {:#?}", dirs);
    assert_eq!(dirs[0].target, Some("target".into()));
    assert_eq!(dirs[0].level, LevelFilter::INFO);
    assert_eq!(dirs[0].span, Some(span_name.into()));
}

#[test]
fn parse_directives_with_invalid_span_chars() {
    let invalid_span_name = "]{";

    let dirs = parse_directives(format!("target[{}]=info", invalid_span_name));
    assert_eq!(dirs.len(), 0, "\nparsed: {:#?}", dirs);
}
