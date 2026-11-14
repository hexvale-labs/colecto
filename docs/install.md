# Installing Colecto

Download the file for your system from the
[latest release](https://github.com/hexvale-labs/colecto/releases/latest). Files are
named `colecto-<version>-<platform>-<architecture>.<ext>`. Check a download against
`SHA256SUMS.txt` with `sha256sum -c SHA256SUMS.txt --ignore-missing`.

Windows installers lack a trusted code-signing certificate, and Android APKs use a
temporary signing key. Releases do not include macOS or iOS builds. Platform-specific installation limits are described below.

| System | File | Notes |
| --- | --- | --- |
| Linux | `.deb`, `.rpm` or `.AppImage` | Install with your package manager, or `chmod +x` the AppImage and run it |
| Windows | `-setup.exe`, or `.msi` on x64 | SmartScreen: choose **More info → Run anyway** |
| Android | `.apk` (`arm64` for most phones) | Allow installing from your browser or file manager. Each release is signed with a new throwaway key, so uninstall the old version before updating |

Architectures: `x64` (Intel/AMD 64-bit) and `arm64` (Apple Silicon, Snapdragon,
phones). There are no 32-bit builds.

Mobile builds are new and lightly tested. To keep mail flowing while your devices are
off, [host the daemon](self-hosting.md) and connect the apps to it.

## Apple builds

Release CI does not build macOS or iOS: the builds could not be signed or notarized,
they doubled CI time, and the maintainer has no Apple device to test them on. The
source still supports both (the Tauri app and the Swift half of
`crates/colecto-native/`), so you can build them yourself on a Mac with Xcode. Neither
platform is tested. Build the UI first (`cd ui && bun install && bun run build`), then
work in `crates/colecto-app/`:

| Target | Build | Notes |
| --- | --- | --- |
| macOS | `bunx @tauri-apps/cli@2.12.4 build --bundles dmg` | Gatekeeper may block the unsigned, unnotarized app; Apple's [manual opening procedure](https://support.apple.com/en-ie/102445) uses **System Settings → Privacy & Security → Open Anyway** |
| iOS | `ios init`, then `ios build` with the same CLI | Xcode calls back into the CLI as `bun tauri`, so add a `package.json` with a `tauri` script first. Sign with your own certificate and provision it for your device; see Apple's [device distribution documentation](https://developer.apple.com/documentation/xcode/distributing-your-app-to-registered-devices) |

The last release workflow that built both, including an unsigned `.ipa`, is the `ios`
job and the `macos` desktop entries of `.github/workflows/release.yml` at commit
`b42c216`.

On Linux, Settings → Connection can install the mail and desktop notification user
services using the currently running executable, including an AppImage. Keep that
file in place after installing services. See [self-hosting](self-hosting.md#install-background-services-from-the-app)
for generated units, private socket connections and service controls.
