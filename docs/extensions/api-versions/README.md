# Extension API versions

How the extension API is versioned, when to add a version and what an agent changes
when it does. The registry `crates/colecto-extension-api/api-versions.json` is the
source of truth. The pages here explain it; where they disagree, the JSON wins.

## Two kinds of version

| Version | Between | Who numbers it | Where |
| --- | --- | --- | --- |
| **API version** | An extension and Colecto (manifest, capabilities, Wasm ABI, operations, limits) | Colecto, sequentially: 1, 2, 3, … | `api_version` in `extension.toml`; the registry |
| **Bus version** | One extension and another (bus requests and events) | Each extension, any way it likes | The request or event name, such as `v1/add_button` |

They are not interchangeable. This page covers API versions. For bus versions see
[Extension bus](../README.md#extension-bus) and
[Bundled bus surfaces](../bundled-bus.md).

## Files

| File | Contents | Embedded in |
| --- | --- | --- |
| `crates/colecto-extension-api/api-versions.json` | Every API version: `version`, `breaking`, `released_in` (`null` until it ships), `capabilities` first available in it, `changes` (one line each) | Host, `cargo colecto` |
| `crates/colecto-extension-api/api-support.json` | `discontinued`: each API version a release can no longer serve, with that `release` tag | Host, `cargo colecto` |
| `docs/extensions/api-versions/API-<n>.md` (`API-<n>-breaking.md` when `breaking` is true) | What changed in version `n`, written from [EXAMPLE.md](EXAMPLE.md) | Docs only |
| `.github/release-notes/pending.md` | Release-note text for API changes merged before a release | Release notes |

Tests in `crates/colecto-extension-api/src/versions.rs` fail when versions skip a
number, an unreleased version comes before a released one, a version has no page (or a
page has no version), a page's `-breaking` suffix disagrees with `breaking`, an
extension capability is missing from the registry or listed twice, or a discontinued
version was never released.

## How the Host uses them

| Manifest `api_version` | Result |
| --- | --- |
| Supported (in the registry, not discontinued) | Loads. Capabilities newer than the version are rejected (`<capability> needs api_version = N or later`) |
| Discontinued | Install is refused. Review returns `discontinued` with the release tag, and the UI shows a dialog linking to `https://github.com/hexvale-labs/colecto/releases/tag/<release>#breaking-api-changes`. An already installed package stays listed, is never loaded, cannot be enabled, and can be removed. The UI shows the same dialog once per extension at startup |
| Newer than the newest registry entry | Refused: "Update Colecto to use it" |
| `0` | Refused as unsupported |

Bundled extensions and the examples go through the same check, so they must target a
supported version.

## When to add a version

Every change to what extensions can rely on adds exactly one version, even a single
new capability, operation, manifest field or relaxed limit. Internal changes that an
extension cannot observe add none.

If the newest version has `released_in: null`, it has not shipped yet: add your change
to it instead of opening another. Otherwise open the next number. The release that
ships a version sets its `released_in`.

## What to change when you add a version

1. Add or extend the entry in `api-versions.json`: new capabilities in `capabilities`,
   everything else as a line in `changes`, and `breaking: true` if any change is
   breaking (see [EXAMPLE.md](EXAMPLE.md) for what counts).
2. Gate the new behaviour on the caller's `api_version` in the Host where an older
   extension could otherwise observe it. Capabilities are gated automatically from the
   registry; anything else needs a check, such as `DOTTED_BUS_NAMES_SINCE` in
   `crates/colecto-package/src/manifest.rs`.
3. Write `API-<n>.md` (or `API-<n>-breaking.md`) from [EXAMPLE.md](EXAMPLE.md).
4. Update the contract pages that own the topic ([extension contract](../README.md),
   [guide](../guide.md), [Rust SDK](../rust-sdk.md)) and say which version adds it.
5. If you are not shipping a release, add the release-note text to
   `.github/release-notes/pending.md`.

### Bump only the extensions that need it

Do not move every bundled extension and example to the newest version. Versions exist
so old targets keep working. Raise `api_version` only in the extension that uses the
new feature. For example, if a new capability is only needed by the composer, change
only `ui/src/extensions/composer/extension.toml`.

## Changing signatures

Do not be afraid to change an existing operation, field or ABI function. Adding a
`request2` next to `request` because `request` needs a different shape is almost always
wrong: it leaves two ways to do one thing forever. Change `request` in a new version
instead, and mark that version breaking.

If you really believe a parallel `request2` is the right call, ask the human
maintainer before adding it.

A breaking change does not drop older versions by itself. Prefer to keep serving the old
shape to extensions that target older versions, by branching on their `api_version` in
the Host. Discontinue a version only when the Host truly cannot keep its contract any
more, for example because a security fix removes something it promised.

## Discontinuing a version

1. Add `{"version": N, "release": "vX.Y.Z"}` to `api-support.json`, where `vX.Y.Z` is
   the release that drops it.
2. Move any bundled extension or example off that version.
3. Say so in that version's page under Notes.
4. That release's notes must have a `## Breaking API changes` section. It needs a
   subsection per discontinued version (`### API N`) that explains exactly why
   the version could not be kept and what extension authors should change. The error
   dialog links to this section.
