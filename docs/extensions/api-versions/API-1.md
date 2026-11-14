# API version 1

## Additions

- The first extension contract, documented in the [extension contract](../README.md)
  and the [guide](../guide.md):
  - Manifest fields: `id`, `name`, `description`, `author`, `api_version`, `slot`,
    `z_index`, `capabilities`, `[[settings]]` and `[bus]`.
  - Slots `shell`, `mailbox`, `reader`, `composer`, `accounts` (bundled only), `panel`
    and `overlay`.
  - Capabilities `account.read`, `messages.list`, `messages.read`, `drafts.list`,
    `drafts.read`, `drafts.write`, `messages.send`, `messages.label`,
    `messages.organize`, `window.overlay` and `extensions.bus`.
  - Wasm imports `colecto.action`, `colecto.request` and `colecto.emit`; exports
    `memory` and `run`.
  - `colecto.request` operations named after the capabilities above, plus
    `settings.read`, `bus.request` and `bus.emit`.
  - Panel views, overlay frames and sprites, owner settings and the extension bus.

## Breaking changes

- N/A

## Notes

API 1 covers the alpha (v0.1.0 to v0.5.3). During that time it changed in place
without new version numbers. The bus, settings and overlays all arrived inside API 1.
From [API 2](API-2.md) on, every change gets its own number.
