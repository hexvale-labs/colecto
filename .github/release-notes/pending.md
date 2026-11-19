<!--
Release-note text for changes merged into develop but not yet released, mainly
extension API versions (see docs/extensions/api-versions/README.md). Whoever ships the
next release moves this text into .github/release-notes/vX.Y.Z.md and empties this
file back to this comment. Never publish this file itself.
-->

## Extension API 2

- `[[settings]]` tables take an optional `section` (`appearance` or `notifications`) that
  shows the setting in that core Settings section instead of the extension's own page.
  Extensions on API 1 keep working unchanged.
- The icon `sparkles`, and `menu_button` now draws its `icon`. See
  [API 2](https://github.com/hexvale-labs/colecto/blob/main/docs/extensions/api-versions/API-2.md).
