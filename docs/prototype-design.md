# Local email prototype

## Intent and scope

Build the local email demo selected by the maintainer: browse sample mail,
read a message, compose a draft, save it, and reopen it after restarting the app.
The demo must be runnable from the repository with documented `just` commands.

## Approaches

- A frontend-only mock is quickest but would put draft ownership outside Rust
  core services, contrary to the project architecture.
- A Svelte frontend with a local Rust HTTP host exercises the intended ownership
  boundaries and runs without native packaging dependencies. Recommended.
- A Tauri desktop app adds native packaging and prerequisites before the demo
  needs any native capability. Defer it to the first native platform slice.

The maintainer selected the local demo scope. The local HTTP host is the
implementer's proposed platform choice, subject to approval of this design.

## Behavior

- Start with a clearly labeled demo account and deterministic sample messages.
- Show Inbox and Drafts navigation, a message list, and a reading/composing pane.
- On narrow screens, show the list or detail with an explicit back control.
- Render email as plain text. Do not interpret message content as HTML.
- Compose recipient, subject, and body; explicitly save drafts and show success
  only after the core confirms persistence. Warn before abandoning unsaved edits.
- Reopen saved drafts and edit them. Preserve entered text on save failures.
- Handle loading, empty lists, unknown messages, and storage errors visibly.
- No sign-in, real receiving/sending, attachments, synchronization, or third-party
  extension loading. The interface must not suggest that mail can be sent.

## Boundaries and persistence

Rust core services own sample messages, draft validation, IDs, and local storage.
Use a versioned JSON draft file in an explicit local data directory. Save through
a temporary file and atomic replacement; never overwrite an unreadable or
unsupported existing store with an empty one.

Define only the typed, versioned service contracts consumed by this demo in
`colecto-extension-api`. A loopback-only Rust Host serves the built Svelte app
and mediates the bundled inbox/reader/composer modules through those contracts.
Validate requests at that boundary and restrict mutation requests to the local
application origin. Bundled modules use a supplied service client rather than
direct database access. No untrusted extension sandbox is claimed.

Use Svelte with Vite for presentation, accessible native controls, visible focus,
responsive layout, and no decorative motion. No UI framework is necessary.

## Verification

Run `just verify` for the Rust workspace and frontend checks/build through
justfile recipes. Test core reading and draft creation/update/reload, failed or
corrupt storage behavior, and Host request validation. Exercise the running
HTTP slice to verify that saved drafts survive a Host restart. Check the UI
flows with an available local browser test runner; report any unverified visual
or real-device behavior accurately.
