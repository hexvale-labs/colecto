# Make your own extension

A walkthrough from an empty folder to an installed pane. The exact rules are in the
[extension contract](README.md); the complete example is
`docs/extensions/examples/hello-pane/`.

## 1. Choose capabilities

Start with the smallest set that does the job: `account.read` for connection status,
`messages.list` and `messages.read` for mail, `drafts.*` for drafts, `messages.send`
to deliver, `messages.label` for labels, `extensions.bus` to talk to other extensions.
The [capability table](README.md#capabilities) lists them all.

The owner consents once, when installing: an enabled extension has every capability it
declares on every account, a disabled one has none.

## 2. Write a manifest

Create `manifest.toml`:

```toml
id = "hello-pane"
name = "Hello pane"
description = "Shows which account is selected and counts presses of a button."
author = "Your name"
api_version = 1
client = "client/main.js"
capabilities = ["account.read"]

[pane]
icon = "extension"
```

IDs use lowercase ASCII letters, digits and hyphens. `[pane]` gives the extension one
workspace pane, whose tab shows its `name`. Use `[settings_page]` instead (or as well)
for a section in Settings. Unknown fields are rejected.

## 3. Write the client part

The client part is one classic script (bundle it if you use modules, for example
`esbuild --bundle --format=iife`). It runs in a Web Worker with no DOM and no network.
One global, `colecto`, connects it to Colecto:

```js
let presses = 0;
function render() {
  colecto.send({ t: 'render', surface: 'pane', tree: {
    type: 'stack', gap: 8, children: [
      { type: 'heading', level: 2, text: 'Hello' },
      { type: 'button', id: 'press', label: `Pressed ${presses} times` },
    ],
  } });
}
colecto.onmessage = (message) => {
  if (message.t === 'start') render();
  if (message.t === 'event' && message.node === 'press') { presses += 1; render(); }
  if (message.t === 'ping') colecto.send({ t: 'pong', n: message.n });
};
```

- **Draw** with component trees: `render` replaces a surface, `patch` changes parts of it.
  The [components](README.md#components) are drawn by Colecto with its own styles; you
  never write HTML.
- **React** to `event` messages: a button press, a submitted form, a changed field.
  Hover, focus, scrolling and typing stay in the renderer.
- **Fetch data** with `call`: `{t: 'call', id, method: 'GET', path: '/account'}` comes back
  as `{t: 'reply', id, ok, data}`. Colecto adds your credential and the selected account;
  the Host checks your capabilities.
- **Answer `ping`** with `pong`, or the watchdog stops you after about 10 seconds.
- **Follow the owner**: `start` carries the selected account, the [theme
  tokens](ui-model.md#theme-tokens) and your settings; `account`, `theme` and `settings`
  arrive when they change.

An uncaught error or an invalid message stops the extension; its pane then shows
"*name* stopped" with **Restart**.

For heavy work, ship a Wasm module under `client/` and instantiate it from your script
with `colecto.file('client/parser.wasm')`, or make a `.wasm` file the entry itself (the
[Wasm client ABI](ui-model.md#wasm-client-abi)).

## 4. Package and install

A package is a zip of `manifest.toml`, `client/` and optional `assets/` images. In this
repository, `just build extension` packages the example as
`target/extensions/hello-pane.colectoext`; elsewhere, any zip tool works.

Start Colecto (`just run`), open **Settings → Extensions**, pick the package (or drop it
on the page) and press **Install**. The warning lists what it asks for and unlocks after
five seconds. The pane joins the reader's group; drag its tab anywhere.

To debug, open the browser's developer tools: the Worker appears under its own frame,
`<name> runtime`, where `console.log` output shows.

## 5. Add owner settings

Declare settings in the manifest and Colecto draws the page for you:

```toml
[[settings]]
key = "greeting"
label = "Greeting"
type = "text"
default = "Hello"
```

The values arrive in `start` as `message.settings`, and again as a `settings` message
whenever the owner changes them. The [contract](README.md#extension-settings) lists the
types and limits.

## 6. Run something without a window

Background work that must happen with no window open, such as answering bus requests,
belongs in a Wasm service part declared with `service = "service.wasm"`. It runs in the
Host with a fresh sandboxed instance per call. Write it in Rust with the
[Rust SDK](rust-sdk.md), which also packages your `client/` folder.

## 7. Talk to another extension

Declare `extensions.bus`. Ask another extension with a `call` to
`POST /bus/<bus>/request` and `{name, payload}`; the answer is `{data}`, or `null` when
nobody answered. Requests to your own bus arrive as `bus.request` messages; answer with
a `call` to `POST /bus/replies/<id>` and `{data}`. Events you subscribe to under
`[bus] subscribe` arrive as `bus.event`.

Name requests and events with a whole-number version first, such as `v1/example`. See
the [bus contract](README.md#extension-bus) and the
[bundled surfaces](bundled-bus.md).

## API version policy

Every extension targets one `api_version`. Versions count 1, 2, 3, …, and every change
to the extension API adds one, so an extension keeps working on the version it targets.
Move to a newer version only when you need something it adds; each version's page in
[API versions](api-versions/README.md) says what changes.

If a release can no longer serve an old version, for example after a security fix, it
discontinues it. Colecto then refuses to install or run extensions that target it, and
links to that release's `Breaking API changes` notes, which explain why.
