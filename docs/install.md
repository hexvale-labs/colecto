# Installing Colecto

Download the archive or package for your system from the
[latest release](https://github.com/kogical/colecto/releases/latest). Files are named
`colecto-<version>-<platform>-<architecture>.<ext>`. Check a download against
`SHA256SUMS.txt` with `sha256sum -c SHA256SUMS.txt --ignore-missing`.

Desktop bundles lack a trusted code-signing certificate, and Android APKs are signed with
Colecto's own key, not through Google Play.

| System | File | Notes |
| --- | --- | --- |
| Linux (`x64`, `arm64`) | `.tar.gz` | Extract and run `./colecto` (includes `lib/libcolecto_ffi.so`, `colecto-daemon`, `colecto-mcp`, and `Colecto.desktop`). Arch users can also install `colecto` or `colecto-bin` from the AUR. |
| Windows (`x64`) | `.zip` | Extract and run `colecto.exe` (includes `colecto_ffi.dll`, `colecto-daemon.exe`, and `colecto-mcp.exe`). SmartScreen: choose **More info → Run anyway**. |
| macOS (`arm64`) | `.tar.gz` | Extract and move `Colecto.app` to Applications. Gatekeeper may block the unsigned app; open it via **System Settings → Privacy & Security → Open Anyway**. |
| Android (`arm64`, `x64`) | `.apk` | Allow installing from your browser or file manager (`arm64` for most phones). Releases share one signing key so new APKs update in place. |
| Web | `-web.tar.gz` | Static Flutter Web bundle, also built into the `ghcr.io/kogical/colectod` Docker image. |

There are no 32-bit builds. For iOS, build from source on a Mac with Xcode (`cd app && flutter build ios`).

To keep mail syncing while your devices are off, [host the daemon](self-hosting.md) and
connect the Colecto apps to it.
