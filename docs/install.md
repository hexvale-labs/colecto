# Installing Colecto

Download the file for your system from the
[latest release](https://github.com/hexvale-labs/colecto/releases/latest). Files are
named `colecto-<version>-<platform>-<architecture>.<ext>`. Check a download against
`SHA256SUMS.txt` with `sha256sum -c SHA256SUMS.txt --ignore-missing`.

Colecto is not code-signed: signing certificates cost money the project does not have.
Your system will warn you the first time. The steps below get past that.

| System | File | Notes |
| --- | --- | --- |
| Linux | `.deb`, `.rpm` or `.AppImage` | Install with your package manager, or `chmod +x` the AppImage and run it |
| Windows | `-setup.exe`, or `.msi` on x64 | SmartScreen: choose **More info → Run anyway** |
| macOS | `.dmg` | Drag Colecto to Applications, then run `xattr -cr /Applications/Colecto.app` once, because macOS reports unsigned downloads as "damaged" |
| Android | `.apk` (`arm64` for most phones) | Allow installing from your browser or file manager. Each release is signed with a new throwaway key, so uninstall the old version before updating |
| iOS | `.ipa` | Unsigned; install with a sideloading tool such as AltStore or Sideloadly, which signs it with your Apple ID |

Architectures: `x64` (Intel/AMD 64-bit), `arm64` (Apple Silicon, Snapdragon, most
phones), `armv7` and `x86` (older Android devices).

Mobile builds are new and lightly tested. To keep mail flowing while your devices are
off, [host the daemon](docker.md) and connect the apps to it.
