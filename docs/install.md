# Installing Colecto

Download the file for your system from the
[latest release](https://github.com/hexvale-labs/colecto/releases/latest). Files are
named `colecto-<version>-<platform>-<architecture>.<ext>`. Check a download against
`SHA256SUMS.txt` with `sha256sum -c SHA256SUMS.txt --ignore-missing`.

Windows installers lack a trusted code-signing certificate, and Apple builds lack
Developer ID signing or device provisioning. Android APKs use a temporary signing
key. Platform-specific installation limits are described below.

| System | File | Notes |
| --- | --- | --- |
| Linux | `.deb`, `.rpm` or `.AppImage` | Install with your package manager, or `chmod +x` the AppImage and run it |
| Windows | `-setup.exe`, or `.msi` on x64 | SmartScreen: choose **More info → Run anyway** |
| Android | `.apk` (`arm64` for most phones) | Allow installing from your browser or file manager. Each release is signed with a new throwaway key, so uninstall the old version before updating |

Architectures: `x64` (Intel/AMD 64-bit) and `arm64` (Apple Silicon, Snapdragon,
phones). There are no 32-bit builds.

Mobile builds are new and lightly tested. To keep mail flowing while your devices are
off, [host the daemon](self-hosting.md) and connect the apps to it.

## Experimental Apple builds

CI builds macOS `.dmg` and unsigned iOS `.ipa` artifacts. Neither platform is
currently supported or validated on physical hardware.

macOS does not always require a paid Developer ID signature to run an app, but
Gatekeeper can block these unnotarized downloads. Where available, Apple's
[manual opening procedure](https://support.apple.com/en-ie/102445) uses
**System Settings → Privacy & Security → Open Anyway**. We cannot guarantee
that a particular unsigned build will launch.

An unsigned IPA cannot run as downloaded on a standard iPhone. It must first be
signed with a suitable certificate and provisioned for the device; Apple's
[device distribution documentation](https://developer.apple.com/documentation/xcode/distributing-your-app-to-registered-devices)
describes those requirements. Producing an IPA is not evidence of working iOS support.
