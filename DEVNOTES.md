This is an unstructured dumping ground for notes during development.

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
