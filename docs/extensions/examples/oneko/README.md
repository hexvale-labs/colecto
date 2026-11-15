# Oneko

A pixel cat on a `ui.canvas` overlay: it runs toward the pointer, sits down nearby and
naps when nothing happens. It shows the [canvas overlay](../../README.md#canvas-overlay):
`[overlay] z_index`, an `OffscreenCanvas` drawn from the Worker, `pointer` messages,
the `frames` fallback, and a sprite loaded from `assets/` with `call` and `bytes: true`.

| File | Contents |
| --- | --- |
| `manifest.toml` | Capabilities (`ui.canvas` only), the overlay and three range settings |
| `client/main.js` | The whole cat, as one classic script |
| `assets/oneko.png` | The sprite sheet |

`just build extension` packages it as `target/extensions/oneko.colectoext`;
`ui/tests/extension-runtime.spec.ts` installs that package and checks the cat follows
the pointer without taking clicks.

The 32×32 sprite sheet is adapted from
[adryd325/oneko.js](https://github.com/adryd325/oneko.js), copyright 2022 adryd, MIT
licensed (see LICENSE-oneko). The original GIF was converted losslessly to PNG.
