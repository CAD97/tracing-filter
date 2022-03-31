# tracing-filter

A system for filtering/querying structured tracing records.

Very much in-progress. **Things are in various states of (non)functional**.

Currently only targets filtering events as they are collected. However, I would
*like* to support filtering recorded events as well.

## Filters

When you construct a `Filter` type, you determine which kind of filters it
supports. You cannot mix different kinds of filters in one filtering layer.

Note that a complicated filter can slow down your tracing performance, so
using a filter from an untrusted (i.e. user) source is typically not
recommended. This is particularly important for legacy and query filters,
however; because they use [regex-automata](https://lib.rs/crates/regex-automata)
to implement unbuffered regex filtering, they are vulnerable to unbounded regex
compilation time à la [CVE-20222-24713]. Simple filters do not do unbuffered
regex evaluation, so instead use the regex crate directly, which has mitigated
this issue.

[CVE-20222-24713]: https://github.com/rust-lang/regex/security/advisories/GHSA-m5pq-gvj9-9vr8

### Simple filters

tracing-filter is 99.999% compatible with [env_logger](lib.rs/env_logger)'s
filter syntax. As such, you can write simple filters the way you always have:

- `warn` — filter to only events of level `WARN` or `ERROR`
- `my_app=debug` — filter to `DEBUG` or higher events only from `my_app`
- `warn,my_app::module=trace` — get warning events and trace `my_app::module`
- `off` — disable all logging
- `debug/foo` — filter to `DEBUG` or higher events whose message contains "`foo`"

In general, the syntax is `target=level/regex`. An event is included if its
target *starts with* the listed `target`, its level passes the `level` filter,
and its message matches `regex`. With the env_logger crate, the regex string
is a simple substring match if you don't enable the `regex` feature; with our
simple filters,

**This should be 99%<sup>†</sup> functional in the `tracing_filter::simple` module.**

<sup>†</sup>: tracing does not allow filtering on events' fields' contents
[yet](https://github.com/tokio-rs/tracing/pull/2008). tracing-filter chooses to
just siliently ignore the regex filter for the time being (but it does validate
the filter).

### Legacy filters

The filter syntax supported by tracing-subscriber@0.3's `EnvFilter`, complete
with all of its p̛̭a͖͕ŕ̯̪̥͈̠̙̣s͙̪̮̟͠i̥̞̠n͍̙̭͡g̸̜̤̦̤̳͍ ͓͜ẉ̨̳̠̗̗i̱t͚̹͉̯h̢̩̤̹͙̩͙ ̪̻͈r̻̙̥̭̯̫e̮̭̞̣̮͕̪g҉̦͚̬̖e͇̕x̛͖̣̮̞̜ͅ "peculiarites"; 100% bug-for-bug compatible.

As such, you can use all of the filters that you have been using:

- `warn` — filter to only events of level `WARN` or `ERROR`
- `my_app=debug` — filter to `DEBUG` or higher events only from `my_app`
- `warn,my_app::module=trace` — get warning events and trace `my_app::module`
- `off` — disable all logging
- `[span]=debug` — filter to `DEBUG` or higher events inside a span named `span`
- `[{field}]` — filter to events with a field `field` or within a span with name `field`
- `[{key=val}]` — filter to events within a span with field `key` that matches the regex `val`
- `[{key=0}]` — filter to events within a span with field `key` that recorded a number that equals `0`
- `[{key=true}]` — filter to events within a span with field `key` that recorded a boolean value of `true`
- `target[span]` — filter to events within a span with target `target` and name `span`

In general, the syntax is `target[span{field=value}]=level`.

**This should be 100% functional in the `tracing_filter::legacy` module.**

### Query filters

Query filters are tracing-filter's way of selecting events and taking advantage
of tracing's structured events. Query filters are a 99% superset of simple
filters; specifically, for each `,` separated directive, it's treated as a query
filter if and only if it starts with `(`; otherwise it is treated as a simple
filter.

**This is still undergoing design work.**

## Why not use tracing-filter?

- tracing-filter is highly experimental
- tracing-filter is not officially supported by the tracing team
- tracing-filter is not published to crates-io
- tracing-filter works with the unpublished tracing 0.2.0 ecosystem

## Why use tracing-filter?

- More configurable than tracing-subscriber@0.3's `EnvFilter`
- You want your runtime filter syntax to work for serialized event queries
- You like the author and want them to feel proud of themself
- We have nice [miette](https://lib.rs/miette)-powered errors :smile:
