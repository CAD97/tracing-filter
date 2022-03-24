This is an unstructured dumping ground for notes during development.

## The problems with spans (2022-03-23)

My initial filter syntax design was focused on filtering of _events_ in an event
stream, e.g. as observed by the fmt layer's output. However, tracing's model is
richer than that, and filtering spans is also necessary for a proper filter. In
fact, in construction of this language, I misunderstood what the env filter
`target[span]` did. I thought it enabled events within `target` within a span
named `span`, but in fact it enables all events within a span named `span`
which is itself marked with `target`.

The difficulty in making a nice-to-use format is that `target` enables both
spans within `target` and events within `target`. The 99% case will want to
enable both together. However, separation has reasonably simple user stories as
well; consider enabling all spans for a crate, but only events within a specific
module (i.e. target).

One idea I had was to use `?span#target` to specify span name/target together
(basically stealing URL fragment syntax for it). That could allow the symantics

- `target`: spans and events with target `target`
- `?span`: spans with name `span`
- `#target`: spans with target `target`
- `* #span`: events within a span with target `target`

Also, I think I want to inverse the current CSS inspired `?span > ?span` to be
`target ?span < ?span`, to maintain a consistent inside-out order. (Perhaps `+`
can be used here? But the inside-out order seems pertinent.)

Combined with a rule that any spans used to enable events are enabled, this
seems somewhat reasonable, at least.

## Memory filter use case (2022-03-19)

For recorded event filtering, it would certainly also be useful to be able to
filter e.g. a json serialized event stream and only have to process/deserialize
a single event at a time. This can be done with e.g. json db queries, but using
the same filter language as runtime filters is beneficial, and the domain
knowledge added makes filtering easier.

## Event field matching (2022-03-19) !important

I believe filtering on event fields to be impossible with today's `Subscribe`
(published `Layer`) design. The reason is that `Subscribe::enabled` gets only
`Metadata` and `Context`, and the recording of fields at `Subscribe::on_record`
only happens after `enabled` is determined.

**This could be addressable by making `on_event` return `ControlFlow`**; the
`Layered` collector would then only continue recording an event if subscribers
report they want to `ControlFlow::Continue` to do so.

## Span field matching (2022-03-18)

Per my reading of tracing_subscriber::EnvFilter, it matches fields on entry
rather than recording and matching at filter time. This is good! AIUI it's an
optimization that allows a) the field matching to be memoized and b) a negative
filter to early-cancel contained spans/events. Unfortunately... more complicated
queries such as [`(my_crate ?span_a ?span_b)=TRACE`][nested-span-filter] don't
lend themselves as well to such caching. It's *possible*, and perhaps worth
doing, but basically requires making an automaton to handle more complex cases
which the query language supports, like `((?a ?b | ?a > ?c) & ?d)`. Plus, I
would like to support filtering recorded spans (e.g. [tracing-memory], another
semi-abandoned project of mine), and those don't really have the same enter/exit
behavior... but maybe I can just "replay" the events to filter them through a
memoized aproach, once it exists?

I think, first-pass, proof-of-concept, serialize span fields into `Extensions`
and do a full match on each event, rather than putting the development effort
into generating the automaton while the project is still experimental.

[nested-span-filter]: https://discord.com/channels/500028886025895936/627649734592561152/954104152059940944
[tracing-memory]: https://github.com/CAD97/tracing-utils/tree/main/libs/tracing-memory

## Static directive optimization (2022-03-18)

We completely punt on the static directive optimization that tracing_subscriber
EnvFilter has for the time being. This will almost certainly need to be looked
into at a later point to match env_logger/current perf for statically disabled
events. (Note "static" here means always for the collector, not compile-time.)
Callsite caching of static directives is certainly an important optimization.

## Misc. notes (2022-03-17)

- Nested span matching is based on CSS selectors; `element element` is
  transitive contains; `element > element` is direct contains. However, there's
  no way to notate root ("`> element`", maybe? Makes the grammar awkward[^1]),
  and while that's not something CSS wants, it'd be useful for us.
- Field syntax should be ready for `valuable::Structable`.
  - Proposal: `valuable::Listable` can be handled by `[]`
  - Proposal: `valuable::Enumerable` can be handled by `= Name? Fields`
  - Weak proposal: `valuable::Tuplable` *could* be directly handled by `(,)`,
    but I'm not *super* happy with that solution since `()` is semantic for
    queries outside of `{}`.
  - I have no idea how to handle `valuable::Mappable` in a not-bad way.
  - All of this depends on how exactly tracing `valuable` support pans out.
    I probably shouldn't bother with implementing nested `Field`s yet, even.
- Field presence without a comparison value just asks for its presence.
- It'd be nice to provide a translation to JSONiq or similar for the JSON
  event formatter.
- The query language technically isn't a query language AIUI, as it doesn't
  return structured results; it only offers filtering. SQL for tracing events
  is a much bigger task than I'm personally willing to take on.

## Query language semantic gaps (2022-03-17)

- We probably want a way to match `my_app` but not `my_app::module`. This is a
  concern even if `tracing` is adjusted to not match `tracing_filter`. Proposal:
  - String targets match the target exactly. Rationale: module nesting is common
    for `module::path` style automatic targets, but if someone specifies a
    custom target that does not meet this convention, they probably aren't using
    nesting and matching the exact specified target would work correctly.
  - `#my_app` to match exactly. Rationale: `#` is used to make strings "more
    literal", so `#my_app` would mean the pattern `my_app`, but "more literal".
    This would also mean that `"my_app"` would get the module matching behavior,
    but `#"my_app"#` would be an exact match. But maybe this is too mean to the
    parser, since "token kind" isn't LL(1) anymore?
  - Make `my_app` only match the literal target `my_app`, and add a fuzzy match
    syntax. Problem: this deviates greatly from existing practice, probably too
    much. However, it would be more consistent with field names (see next).
- Field names should certainly always be exact matches. This is different and
  somewhat inconsistent with target patterns.
- Matching `field = "string"` should probably be an exact match, as that's what
  `=` logically means. However, env_logger provides a regex match for the event
  message, and that's quite useful for working with less-structured events.
  Proposal: `field ~ "regex"` (or `~=`).
- Do we want a shorthand for `{ message ~ "regex" }`? Proposal: allow a tail
  `~ "regex"` in `Select` to mean a `{ message ~ "regex" }` event query.
- String syntax doesn't provide escapes, which might be surprising?
  I just don't want to support them, though.
- `field.field` shorthand for `field = { field }` seems desirable.

## Query language syntactic gaps (2022-03-17)

As currently specified, the query language uses the ASCII subset of
[UAX31 Pattern Syntax](http://www.unicode.org/reports/tr31/#Pattern_Syntax).
That is, `\t\n\v\f\r ` are considered whitespace, and we reserve ASCII symbols
``!"#$%&'()*+,-./;<=>?@[\]^`{|}~`` for syntax; any other characters are treated
as "pattern" characters. For use convenience, `_:` are also considered pattern
characters; `_` is a valid rust ident char, and `:` shows up in common targets.

The current grammar uses `"#&(),-<=>?{|}`. All of the other syntax characters
``!$%'*+./;[\]^`~`` are reserved and can be given semantics in future updates.
Also, invalid syntax can of course be given meaning.

## Target matching semantics (2022-03-17)

We probably want to keep prefix matching for targets, so that a `my_app` query
returns all events from `my_app`, including ones from modules, so they have
targets like `my_app::error` or `my_app::tracing`. However, it's worth noting
that a simple prefix matcher means that the `tracing` filter will also include
events from `tracing_subscriber` and `tracing_filter`, so perhaps a slightly
different rule is warrented; perhaps `== "my_app" || .starts_with("my_app:")`?
Or perhaps the regex `my_app(?-u:\b)` (any character not in `[0-9A-Za-z_]`
follows.)?
