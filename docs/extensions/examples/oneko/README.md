# Oneko

A no_std Rust guest for API 1. Build with `just build oneko`.
The behavior and original 32×32 sprite sheet are adapted from
[adryd325/oneko.js](https://github.com/adryd325/oneko.js), copyright 2022 adryd,
MIT licensed (see LICENSE-oneko). The original GIF was converted losslessly to PNG.
This is a third-party example, never bundled or installed automatically.

Settings → Oneko changes speed (2–30 px per step, default 10), opacity (20–100%,
default 100) and stopping distance (24–160 px, default 44). Each frame reads its
own persisted settings with one `settings.read` call. Defaults preserve the
original movement and fully opaque sprite.
