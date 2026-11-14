# API version 3

## Additions

- Bus request and event names may contain `.`, as in `v0.1.2/add_button`, both in
  `[bus] subscribe` entries and in `bus.request` and `bus.emit` calls. Extensions on
  API 1 and 2 still get the old character set (lowercase ASCII, digits, `-`, `_`, `/`).
  The Host checks the calling extension's version (`DOTTED_BUS_NAMES_SINCE` in
  `crates/colecto-package/src/manifest.rs`).

## Breaking changes

- N/A

## Notes

Bundled extensions do not need this and stay on API 1: they name bus messages with
whole-number versions only (`v1/`, `v2/`). Third-party authors are strongly encouraged
to do the same. See [Extension bus](../README.md#extension-bus). The
[Rust SDK](../rust-sdk.md#bus-messages) always generates whole-number names
(`v<version>/<name>`) and still rejects `.` in the name it is given.
