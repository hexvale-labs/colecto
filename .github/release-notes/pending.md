<!--
Release-note text for changes merged into develop but not yet released, mainly
extension API versions (see docs/extensions/api-versions/README.md). Whoever ships the
next release moves this text into .github/release-notes/vX.Y.Z.md and empties this
file back to this comment. Never publish this file itself.
-->
## Extension API 2

Mail lists can now reach every folder and older mail. `/messages/refresh` and
`/messages/cache` take any folder id from `/folders`, and `/messages/refresh` takes
`offset` to load an older page. See
[API 2](https://github.com/kogical/colecto/blob/vX.Y.Z/docs/extensions/api-versions/API-2.md).
