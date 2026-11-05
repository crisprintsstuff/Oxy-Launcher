# Oxy Studio

A lightweight, high-performance desktop launcher, updater, and hub for the **Oxy** creative suite, including [OxyBrush](https://github.com/crisprintsstuff/OxyBrush) and [OxyPixel](https://github.com/crisprintsstuff/oxypixel).

Built in Rust with [`eframe`](https://github.com/emilk/egui/tree/master/crates/eframe) / [`egui`](https://github.com/emilk/egui).

---

## Features

- **BozNetwork Portal Design System**: Styled after the BozNetwork Console / Hub design language with Liquid Glass dark navy canvas, App Store Today editorial hero presentation, capsule navigation chips, and pill action buttons (`GET`, `OPEN`, `UPDATE`).
- **Smart App Auto-Discovery**: Automatically detects pre-existing installations on Windows (Registry `HKCU`/`HKLM`, `%LOCALAPPDATA%`, `%ProgramFiles%`, and `PATH`), Linux, and macOS, linking them immediately without requiring a re-download.
- **Manual Executable Linking**: Native file dialog to relocate or link custom portable/standalone executables.
- **Native Windows Installers**: Includes both Inno Setup (`packaging/windows/oxystudio.iss`) and NSIS (`installer/oxystudio.nsi`) installers with custom high-resolution application icons (`oxystudio.ico`).
- **Direct GitHub Releases Integration**: Queries public GitHub releases for official builds, release notes, and assets with no third-party server dependencies.
- **Secure Downloads & Verification**: Validates downloaded release archives against upstream `SHA256SUMS.txt` hashes before unpacking.
- **Non-blocking Downloads**: Asynchronous download and extraction pipeline with real-time progress indicators, transfer rates, and byte counters.
- **Channel Switching**: Easily switch between stable releases and pre-releases (alphas, betas, RCs).
- **Installed Apps Manifest**: Tracks installed versions and local paths, with quick actions to launch, update, open the installation directory, or uninstall.
- **Fast & Lightweight Native GUI**: Fast startup with low resource usage and a clean dark theme matching OxyBrush and OxyPixel.

---

## Workspace Structure

```
oxystudio/
├── crates/
│   ├── oxystudio-core/    # Core engine: GitHub API, downloads, checksums, archive extraction, manifests, process launcher
│   └── oxystudio/         # Native desktop GUI launcher built with eframe/egui
├── Cargo.toml             # Cargo workspace definition
├── Makefile               # Build, format, lint, and test tasks
└── README.md
```

### Supported Oxy Applications

| Application | Category | Repository |
|---|---|---|
| **OxyBrush** | 3D Digital Sculpting & Topology | [`crisprintsstuff/OxyBrush`](https://github.com/crisprintsstuff/OxyBrush) |
| **OxyPixel** | Pixel-Art & Animation Suite | [`crisprintsstuff/oxypixel`](https://github.com/crisprintsstuff/oxypixel) |

---

## Building and Running

### Prerequisites

- Rust 1.85+ (tested on Rust 1.99)
- On Linux, standard graphics libraries (`libx11-dev`, `libwayland-dev`, `libxkbcommon-dev`)

### Run Development Build

```bash
cargo run -p oxystudio
```

### Build Optimized Release

```bash
cargo build --workspace --release
```

The resulting binary will be in `target/release/oxystudio`.

### Tests and Linting

```bash
make check
```

Or individually:
```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## License

Licensed under the MIT License ([LICENSE](LICENSE)).
