# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

People who live in their mail and want a fast, keyboard-friendly client they can
reshape with extensions. (Inferred from the brief and `docs/README.md`; not
confirmed in an interview.)

## Product Purpose

Colecto is an email client (IMAP/SMTP and JMAP; never POP3) with Rust core
services and a Svelte UI built from replaceable bundled extensions. Success means
reading, triaging and writing mail is quick and obvious, and extensions can change
layouts and workflows without risking mail or account integrity.

## Positioning

Deep customisability on a trusted core: the whole UI is extensions using the same
public contracts as third-party ones, and third-party panels run in an isolated
Wasm runtime.

## Capabilities and Constraints

- Desktop (Tauri), browser and mobile targets share one service boundary.
- Accounts, labels, drafts, search, open-message tabs, a command palette, API keys
  and human send approvals exist today.
- Panes can be docked left, center, right or bottom, tabbed together and resized,
  so each pane must work in tall, wide and in-between slots.

## Brand Commitments

- Name "Colecto"; the logo and wordmark live in `docs/branding/`.
- Dark only. See "Light mode" in `DESIGN.md`.

## Product Principles

1. A calm default (list on the left, the thing you're working on beside it) that people can rearrange.
2. Never lose the user's mail or unsaved writing.
3. Keyboard first, but every action is reachable by pointer and touch.
