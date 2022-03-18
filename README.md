# tracing-filter

A system for filtering/querying structured tracing records.

Very much in-progress. **Approximately nothing works yet**.

## Filters

### Simple filters

tracing-filter is 99.999% compatible with [env_logger](lib.rs/env_logger)'s
filter syntax. As such, you can write simple filters the way you always have:

- `warn` — filter to only events of level `WARN` or `ERROR`
- `my_app=debug` — filter to `DEBUG` or higher events only from `my_app`
- `warn,my_app::module=trace` — get warning events and trace `my_app::module`
- `off` — disable all logging
- `trace/foo` — get all events whose message contains "`foo`"

In general, the syntax is `target=level/regex`. An event is included if its
target *starts with* the listed `target`, its level passes the `level` filter,
and its message matches `regex`. The only non-compatible behavior is that to use
a simple filter, the filter must not start with the character `(`. This only
impacts log events that specify a custom `target`. Additionally, `/filter` can
only be used with the `regex` feature; it's ignored otherwise. With the
env_logger crate, it would be interpreted as a simple substring filter.

If the filter is a simple filter, it must entirely be a simple filter.
You may not mix simple filters with query filters.

### Query filters

Query filters are tracing-filter's way of selecting events and taking advantage
of tracing's structured events.

## Why not use tracing-filter?

- tracing-filter doesn't work yet
- You want compatibility with tracing-subscriber@0.2's `EnvFilter`
- You don't want to pull in a nonstandard tracing-subscriber layer

## Why use tracing-filter?

- More configurable than tracing-subscriber@0.2's `EnvFilter`
- You want your runtime filter syntax to work for serialized event queries
- You like the author and want them to feel proud of themself
