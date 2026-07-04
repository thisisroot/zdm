# ZDM

A fast, segmented download manager. ZDM splits a download across multiple
HTTP connections (like IDM/Persepolis), shows genuine per-connection progress,
and supports queues, batch/pattern downloading (`part[01-99].zip`), and
auto-categorized folders.

## Architecture

- **`crates/zdm-core`** — the download engine. Pure Rust, no GUI dependency:
  probes a URL for range support, splits the file into small chunks pulled
  from a shared queue by N concurrent connections, persists enough state
  (`<file>.zdm.json`) to resume after a restart. Has its own test suite
  (`cargo test -p zdm-core`) that runs against a real local HTTP server.
- **`src-tauri`** — the Tauri (Rust) backend: turns engine events into
  `DownloadRecord`s, runs the queue scheduler, persists history/settings to a
  local SQLite database, and exposes commands to the frontend.
- **`src`** — the React + TypeScript UI.

## Developing

```sh
npm install
npm run tauri dev
```

### Platform prerequisites

**Windows**

- [Rust](https://rustup.rs) (MSVC toolchain — the default `rustup` install)
- [Node.js](https://nodejs.org) 20+
- [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/) with the "Desktop development with C++" workload
- WebView2 Runtime (preinstalled on Windows 10/11; Tauri prompts to install it otherwise)

**macOS**

- [Rust](https://rustup.rs)
- [Node.js](https://nodejs.org) 20+
- Xcode Command Line Tools (`xcode-select --install`)

**Linux (Debian/Ubuntu)**

```sh
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev pkg-config libdbus-1-dev
```

## Building installers

```sh
npm run tauri build
```

Produces a native installer for whichever OS you run it on (NSIS/MSI on
Windows, `.dmg`/`.app` on macOS, `.deb`/`.rpm`/AppImage on Linux) — Tauri
builds are not cross-compiled, so getting all three requires running this on
each OS (or using the CI workflow below).

### Building for all three platforms via CI

`.github/workflows/build.yml` builds Windows, macOS, and Linux installers in
parallel on GitHub's own runners. Push this repo to GitHub, then either:

- Push a tag like `v0.1.0` — it builds and attaches installers to a draft GitHub Release, or
- Run the workflow manually from the Actions tab (`workflow_dispatch`)

The builds are unsigned (no code-signing certificates configured), so
Windows SmartScreen and macOS Gatekeeper will warn on first launch until
signing is set up.

## Testing

```sh
cargo test --workspace   # Rust: engine + backend
npx tsc -b               # TypeScript typecheck
```
