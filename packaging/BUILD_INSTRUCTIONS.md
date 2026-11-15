# Build & Packaging Guide — Marmota Vehicle Tester

## Runtime Directory Matrix

The Rust runtime (`src/config.rs::get_runtime_directories`) automatically selects writable storage locations depending on whether the app is running from Cargo dev mode (`cargo run`) or as a standalone packaged `.exe` / `.app`:

| Data Artifact | Dev (`cargo run`) | Windows Packaged (`.exe`) | macOS Packaged (`.app`) | Linux Packaged |
| --- | --- | --- | --- | --- |
| User Config (`config.json`) | `<project>/config.json` | `%APPDATA%\MarmotaVehicleTester\config.json` | `~/Library/Application Support/MarmotaVehicleTester/config.json` | `~/.local/share/MarmotaVehicleTester/config.json` |
| Vehicle Catalog & DB (`marmota_catalog.db`) | `<project>/assets/` | `%APPDATA%\MarmotaVehicleTester\assets\` | `~/Library/Application Support/MarmotaVehicleTester/assets/` | `~/.local/share/MarmotaVehicleTester/assets/` |
| Rotating App Logs | `<project>/logs/` | `%APPDATA%\MarmotaVehicleTester\logs\` | `~/Library/Application Support/MarmotaVehicleTester/logs/` | `~/.local/share/MarmotaVehicleTester/logs/` |
| WebView User Profile | `<project>/webview2/` | `%LOCALAPPDATA%\MarmotaVehicleTester\webview2\` | `~/Library/Application Support/MarmotaVehicleTester/webview2/` | `~/.local/share/MarmotaVehicleTester/webview2/` |

Optional overrides:
- `WT_FORCE_PACKAGED_PATHS=1` (or `MARMOTA_PACKAGED=1`): force packaged AppData/Application Support paths even during testing.
- `MARMOTA_DATA_DIR=<dir>`: redirect all runtime data to a custom writable directory.

## Embedded Static Assets & Windows Resources

- UI HTML/CSS/JS/Locales/PNG flags are compiled directly into the binary via `rust-embed` (`ui/`).
- Starter test missions are compiled into the binary via `include_str!` (`src/starter_missions/`).
- Optional offline `assets-bundle.zip` or `assets.zip` is automatically checked in `<bundle>.app/Contents/Resources/` (macOS) or `<exe_dir>/resources/` (Windows).
- `build.rs` embeds `packaging/windows/app.ico`, `packaging/windows/app.manifest`, and `VS_VERSION_INFO` (`RT_VERSION`) using `rc.exe` / `llvm-rc` when present, or falls back automatically to its built-in pure-Rust 32-bit `.res` generator for macOS/Linux cross-compilation (`cargo-xwin`), alongside PE checksum & hardening flags (`/RELEASE`, `/VERSION:2.0`, `/HIGHENTROPYVA`, `/DYNAMICBASE`, `/NXCOMPAT`, `/CETCOMPAT`, `/PDBALTPATH:%_PDB%`).

## Native Windows `.exe` Release Commands

Prerequisites: Rust Windows toolchain (`x86_64-pc-windows-msvc`), Windows 10/11 SDK, Microsoft Edge WebView2 Runtime (pre-installed on Windows 10/11).

```powershell
# Compile standalone Windows GUI .exe with embedded icon and PerMonitorV2/asInvoker manifest:
cargo build --release

# Output binary:
# target\release\marmota_vehicle_tester.exe
```

## macOS `.app` Release Commands

Prerequisites: Xcode Command Line Tools (`xcode-select --install`), Rust Apple target (`x86_64-apple-darwin` or `aarch64-apple-darwin`).

```bash
# Build release binary (using --remap-path-prefix to omit local build machine paths):
RUSTFLAGS="--remap-path-prefix=$(pwd)=. --remap-path-prefix=$HOME=~" \
  cargo build --release --target x86_64-apple-darwin

# Assemble and codesign .app bundle:
mkdir -p "dist/macos/Marmota Vehicle Tester.app/Contents/MacOS" \
         "dist/macos/Marmota Vehicle Tester.app/Contents/Resources"
cp packaging/macos/Info.plist "dist/macos/Marmota Vehicle Tester.app/Contents/Info.plist"
cp packaging/macos/AppIcon.icns "dist/macos/Marmota Vehicle Tester.app/Contents/Resources/AppIcon.icns"
cp target/x86_64-apple-darwin/release/marmota_vehicle_tester \
   "dist/macos/Marmota Vehicle Tester.app/Contents/MacOS/marmota_vehicle_tester"
chmod +x "dist/macos/Marmota Vehicle Tester.app/Contents/MacOS/marmota_vehicle_tester"
strip -S -x "dist/macos/Marmota Vehicle Tester.app/Contents/MacOS/marmota_vehicle_tester"
xattr -cr "dist/macos/Marmota Vehicle Tester.app"

codesign --force --deep --options runtime \
  --entitlements packaging/macos/entitlements.plist \
  --sign - \
  "dist/macos/Marmota Vehicle Tester.app"
```

## Cross-Compiling Windows `.exe` from macOS (`cargo-xwin`)

Prerequisites: `rustup target add x86_64-pc-windows-msvc`, `rustup component add llvm-tools`, and [`cargo-xwin`](https://github.com/rust-cross/cargo-xwin) (`llvm-lib` symlinked to `llvm-ar` if not already on `PATH`).

```bash
RUSTFLAGS="--remap-path-prefix=$(pwd)=/build --remap-path-prefix=$HOME=/user" \
  TARGET_CFLAGS="/clang:-ffile-prefix-map=$HOME=/user" \
  cargo xwin build --release --target x86_64-pc-windows-msvc

cp target/x86_64-pc-windows-msvc/release/marmota_vehicle_tester.exe \
   dist/windows/MarmotaVehicleTester.exe
```
