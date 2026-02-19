# SlickRun

A fast command launcher for Linux (GNOME/Wayland), inspired by [SlickRun for Windows](https://bayden.com/slickrun/).

Type a keyword, hit Enter, and it runs. That's it.

![Rust](https://img.shields.io/badge/Rust-1.85+-orange) ![Linux](https://img.shields.io/badge/Platform-Linux-blue) ![License](https://img.shields.io/badge/License-MIT-green)

## Features

- **Magic Words** -- define keyword shortcuts for commands, URLs, and scripts
- **Autocomplete** -- type-ahead hints with Tab completion
- **Global hotkey** -- summon the launcher from anywhere (default: Win+Shift+Q)
- **System tray** -- Show/Hide, Settings, and Quit from the tray icon
- **Auto-hide** -- hides when it loses focus
- **Always on top** -- stays above other windows
- **Draggable** -- click and drag anywhere outside the text input
- **Remembers position** -- persists window location between sessions
- **Configurable** -- font color, size, opacity, window dimensions, hotkey
- **Start at login** -- optional autostart via XDG autostart
- **Import/Export** -- backup and restore your magic words as JSON
- **GNOME Wayland support** -- installs a GNOME Shell extension for proper window activation
- **`$W$` substitution** -- pass arguments to magic words (e.g., `g rust traits` -> Google search)
- **`@copy@`** -- special command that copies resolved parameters to clipboard

## Built-in Commands

| Command | Action |
|---------|--------|
| `setup` | Opens the settings window |
| `quit`  | Exits the application |

Any unrecognized input is executed as a shell command (`sh -c "..."`).

## Building

### Prerequisites

Ubuntu/Debian:

```bash
sudo apt-get install -y libgtk-3-dev libayatana-appindicator3-dev
```

Fedora:

```bash
sudo dnf install gtk3-devel libayatana-appindicator-gtk3-devel
```

### Build

```bash
cargo build --release
```

The binary will be at `target/release/slickrun`.

### First Run on GNOME Wayland

SlickRun installs a GNOME Shell extension for window activation. After the first run, **log out and back in once** for GNOME Shell to discover the extension. After that, the global hotkey will properly focus the window.

## Configuration

Settings are stored at `~/.config/slickrun/config.json`.

Open settings by typing `setup` in the launcher, or via tray icon > Settings.

### Settings Tabs

- **Library** -- add, edit, delete, search, import/export magic words
- **Appearance** -- font color, font size, window width/height
- **Options** -- hotkey, start at login, auto-hide, stay on top, opacity

## Creating a Release Binary

To distribute a self-contained binary:

```bash
# Build a statically-linked release binary
cargo build --release

# Strip debug symbols to reduce size
strip target/release/slickrun

# (Optional) compress with UPX
upx --best target/release/slickrun
```

### GitHub Release

1. Tag a version:

```bash
git tag v0.1.0
git push origin v0.1.0
```

2. Create a release on GitHub:

```bash
gh release create v0.1.0 target/release/slickrun \
  --title "SlickRun v0.1.0" \
  --notes "Initial release"
```

Or go to your repo's **Releases** page on GitHub, click **Draft a new release**, select the tag, and upload `target/release/slickrun` as a binary attachment.

### Note on Portability

The binary dynamically links against GTK3 and system libraries. Users need the same runtime dependencies installed:

```bash
# Ubuntu/Debian
sudo apt-get install -y libgtk-3-0 libayatana-appindicator3-1

# Fedora
sudo dnf install gtk3 libayatana-appindicator-gtk3
```

## License

MIT
