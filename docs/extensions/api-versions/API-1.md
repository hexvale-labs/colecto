# API version 1

The beta extension API: everything an extension can rely on when Colecto entered beta.

## Additions

- The client-side extension model, documented in the [extension contract](../README.md)
  and designed in the [extension UI model](../ui-model.md):
  - Manifest fields: `id`, `name`, `description`, `author`, `api_version`, `client`,
    `service`, `capabilities`, `[pane]`, `[settings_page]`, `navigation`, `[overlay]`,
    `[bus]` and `[[settings]]`, with an optional `section` (`appearance` or
    `notifications`) on each setting that shows it in that core Settings section.
  - Packages with `manifest.toml`, `client/`, an optional service module and `assets/`.
  - Capabilities `account.read`, `messages.list`, `messages.read`, `messages.label`,
    `messages.organize`, `drafts.list`, `drafts.read`, `drafts.write`, `messages.send`,
    `extensions.bus`, `ui.frame`, `ui.frame.remote`, `ui.commands`, `ui.app_actions`,
    `ui.toasts`, `ui.contribute`, `ui.canvas`, `approvals.notify`, `releases.read`, `ai.text`,
    `ai.decide` and `ai.configure`.
  - Client parts in a Web Worker with the `colecto` global (`onmessage`, `send`, `file`),
    the Wasm client ABI, per-extension credentials, the shell protocol (`start`,
    `account`, `theme`, `settings`, `event`, `host`, `bus.request`, `bus.event`, `reply`,
    `ping`, `navigate`, `back`, `command`, `app_action`, `toast_action`, `command_query`,
    `command_submit`, `slot`, `canvas`, `resize`, `pointer`; `render`, `patch`, `call`,
    `pong`, `state`, `show`, `leave`, `focus`, `dialog`, `toast`, `toast_dismiss`,
    `commands`, `command_search`, `command_hits`, `app_actions`, `contribute`, `download`,
    `frame`, `menu`), the component vocabulary, contribution slots, a canvas overlay and
    theme tokens.
  - Row context menus: a `list` with `context_menu: true` sends a `context` event, the
    owner answers with a `menu` message of up to 24 items, and the choice comes back as a
    `menu` event.
  - `message_row` takes `starred`; messages carry `thread`, the provider's conversation id.
  - Per-extension key-value storage (`/extensions/{id}/storage/{key}`).
  - Service parts with the Wasm imports `colecto.action`, `colecto.request` and
    `colecto.emit`, answering bus requests and receiving bus events and new-mail
    (`mail`) events; service operations `labels.list`, `storage.get`, `storage.set`,
    `ai.settings` and `ai.decide`; 64 service requests per invocation.
  - Owner settings and the extension bus; bus names may contain `.`.

## Breaking changes

- N/A

## Notes

Colecto's beta started the API over at 1. Every alpha API is gone without compatibility:
the panel and overlay system (v0.1.0 to v0.5.4) and the alpha's client-side APIs 1 and 2
(v0.6.0 to v0.7.x). An alpha package that declares `api_version = 2` is refused as too
new, and one that declares `api_version = 1` must be rebuilt against this page. Any
further change to the extension API adds API 2.
