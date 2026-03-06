# turntouch

A Rust CLI that connects to a [Turn Touch](https://shop.turntouch.com/) Bluetooth LE remote and runs shell commands on button presses.

## Features

- Connects to Turn Touch remotes over Bluetooth LE (V1 and V2 firmware)
- Detects single press, double tap, hold, and multi-button press events
- Runs configurable shell commands for each event
- Automatic reconnection after sleep/wake or signal loss
- Nix flake for reproducible builds

## Requirements

- macOS (uses CoreBluetooth via [btleplug](https://crates.io/crates/btleplug))
- Bluetooth permission granted to your terminal app or the installed .app bundle

## Install

### With Nix

```sh
nix build
# or enter a dev shell
nix develop
cargo build --release
```

### With Cargo

```sh
cargo build --release
```

On macOS you need the Apple SDK and libiconv available. The Nix flake handles this automatically.

## Bluetooth permissions (macOS)

macOS requires Bluetooth access to be explicitly granted. There are two options:

**Option 1** — Grant Bluetooth to your terminal app (e.g. Terminal, iTerm, Ghostty) in:

> System Settings → Privacy & Security → Bluetooth

**Option 2** — Install as a .app bundle:

```sh
turntouch --install
open ~/Library/Application\ Support/TurnTouch.app
```

The first launch will trigger the macOS Bluetooth permission dialog. After granting permission, you can run the binary directly.

## Usage

```sh
turntouch [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `-c, --config <PATH>` | Path to config file |
| `-t, --timeout <SECS>` | Scan timeout in seconds (default: 30) |
| `--install` | Create/update the .app bundle and exit |

The program scans for a Turn Touch remote, connects, and listens for button events indefinitely. It automatically reconnects if the connection drops (e.g. after sleep/wake).

Press `Ctrl+C` to exit.

## Configuration

Config file location: `~/Library/Application Support/turntouch/config.toml` (macOS)

```toml
[north]
press = "open -a Slack"
double = "echo 'North double-tapped'"
hold = "echo 'North held'"

[east]
press = "echo 'East pressed'"

[west]
press = "osascript -e 'tell application \"Music\" to playpause'"

[south]
press = "echo 'South pressed'"

[multi]
press = "echo 'Multi-button press: $TT_DIRECTION'"
```

Each direction supports three event types:

| Event | Description |
|-------|-------------|
| `press` | Single button press |
| `double` | Double tap (within 250ms) |
| `hold` | Long press |

The `[multi]` section fires when 2 or more buttons are pressed simultaneously.

Commands are executed via `sh -c` with these environment variables:

| Variable | Example |
|----------|---------|
| `TT_DIRECTION` | `north`, `east`, `west`, `south`, or `north+east+west+south` for multi |
| `TT_EVENT` | `press`, `double`, `hold`, or `multi` |

If no config file exists, events are logged but no commands are run.

## Protocol

The Turn Touch remote advertises as "Turn Touch" over BLE. Button state is read from a single characteristic:

- **Byte 0**: Inverted button bitmask — `N=0x01, E=0x02, W=0x04, S=0x08`; upper nibble encodes hardware double-click state (V3+ firmware)
- **Byte 1**: `0xFF` if the button is held

Two firmware versions are supported, each with different service/characteristic UUIDs (V1 and V2).

## License

MIT
