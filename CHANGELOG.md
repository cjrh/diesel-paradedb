# Changelog

All notable changes to `diesel-paradedb` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/).

The crate's major version tracks Diesel's: a Diesel 3.x release will
correspond to `diesel-paradedb` 3.x.

## [Unreleased]

## [0.1.0] — 2026-05-19

Initial release.

### Added
- `ParadeQuery` SQL type marker for ParadeDB's `pdb.query` type.
- Infix operators `|||` (`TermMatch`) and `@@@` (`ParadeMatch`), exposed
  as `.term_match(...)` and `.parade_match(...)` fluent methods on
  `Expression`.
- SQL function bindings: `pdb_all()`, `pdb_score(key)`.
- Aggregate / window expressions for `pdb.agg(...)`:
  - `pdb_agg_terms(field, size)` — bucket-count aggregate.
  - `pdb_agg_avg(field)` — average of a numeric fast field.
  - `pdb_agg_stats(field)` — count/min/max/sum/avg in one pass.
  - `pdb_agg_value_count(field)` — non-null value count.
  - `.over_all()` to convert any aggregate to the window form
    `pdb.agg(...) OVER ()`, required for the search-with-facets pattern.

[Unreleased]: https://github.com/cjrh/diesel-paradedb/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/cjrh/diesel-paradedb/releases/tag/v0.1.0
