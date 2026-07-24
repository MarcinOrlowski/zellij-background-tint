# Plugin API: no initial `ModeUpdate` at startup, and `theme_dark` not reflected in plugin-facing `Style`

## Summary

While writing a background plugin that needs the active theme's background colour, I hit two things I can't explain from the docs and would like clarification on:

1. **A plugin receives no `ModeUpdate` at startup.** After `subscribe(&[EventType::ModeUpdate, ...])` in `load()`, `PaneUpdate` and `Timer` events are delivered immediately and repeatedly, but the **first `ModeUpdate` only arrives after the user changes input mode** (e.g. `Ctrl-g`). Until then the plugin has no way to read `ModeInfo.style`.
2. **The configured theme is not reflected in the plugin-facing `Style`.** With `theme_dark "dracula"` set (and no `theme` / `theme_light`), the `ModeUpdate` that eventually arrives reports `style.colors.text_unselected.background = PaletteColor::EightBit(16)` (pure black), not dracula's background (`#282a36`).

Are these expected? And if so, what is the intended way for a plugin to obtain the active theme/`Style` at load time?

## Environment

| | |
|---|---|
| Zellij | 0.44.3 |
| `zellij-tile` | 0.44.0 |
| Plugin target | `wasm32-wasip1` |
| rustc | 1.97.1 |
| OS | Ubuntu 25.10, kernel 6.17.0 |
| `TERM` / `COLORTERM` | `xterm-256color` / `truecolor` |
| Plugin load | background plugin via `load_plugins { ... }` |

## Minimal reproduction

### `src/main.rs`

```rust
use std::collections::BTreeMap;
use zellij_tile::prelude::*;

#[derive(Default)]
struct Probe;

register_plugin!(Probe);

impl ZellijPlugin for Probe {
    fn load(&mut self, _config: BTreeMap<String, String>) {
        eprintln!("[probe] load");
        subscribe(&[EventType::ModeUpdate, EventType::PaneUpdate]);
        request_permission(&[PermissionType::ReadApplicationState]);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::ModeUpdate(mode_info) => eprintln!(
                "[probe] ModeUpdate: text_unselected.background = {:?}",
                mode_info.style.colors.text_unselected.background
            ),
            Event::PaneUpdate(_) => eprintln!("[probe] PaneUpdate"),
            _ => {}
        }
        false
    }
}
```

### `Cargo.toml`

```toml
[package]
name = "probe"
version = "0.1.0"
edition = "2021"

[lib]
# or [[bin]] - either reproduces; this project uses a bin target
path = "src/main.rs"

[dependencies]
zellij-tile = "=0.44.0"
```

### Config (`~/.config/zellij/config.kdl`)

```kdl
theme_dark "dracula"   // no `theme` or `theme_light` set

plugins {
    probe location="file:/absolute/path/to/probe.wasm"
}
load_plugins {
    probe
}
```

### Steps

1. `cargo build --release --target wasm32-wasip1`, copy the `.wasm` to the path above.
2. Start a **new** session from a plain terminal: `zellij`.
3. Split a couple of panes. **Do not change input mode.**
4. `tail -f /tmp/zellij-$(id -u)/zellij-log/zellij.log | grep '\[probe\]'`
5. Now press `Ctrl-g` (enter a non-normal mode) and watch the log again.

## Expected

After subscribing to `ModeUpdate`, the plugin receives an initial `ModeUpdate` reflecting the current mode/`Style` at (or shortly after) `load()`, so it can read the active theme without requiring the user to change modes.

## Actual

- Throughout startup (and for as long as the user stays in the default mode), **only `PaneUpdate` / `Timer` events are delivered** - never `ModeUpdate`.
- The **first `ModeUpdate` appears only after the first input-mode change** (`Ctrl-g`).
- That `ModeUpdate` reports `text_unselected.background = EightBit(16)` (black), despite `theme_dark "dracula"`.

## Log evidence

From an instrumented build of the real plugin (labels differ, behaviour is identical). Session load and the whole startup window contain **no** `ModeUpdate`:

```
19:14:28.086  [bg-tint] load
19:14:28.144  [bg-tint] PaneUpdate: 1 terminal pane(s)
19:14:28.185  [bg-tint] PaneUpdate: 1 terminal pane(s)
   ... (PaneUpdate x42, Timer x12 over the next ~80s) ...
19:15:08.077  [bg-tint] PaneUpdate: 7 terminal pane(s)
--- user presses Ctrl-g here ---
19:15:50.477  [bg-tint] ModeUpdate: raw=EightBit(16) -> rgb=(0, 0, 0)
19:15:50.860  [bg-tint] ModeUpdate: raw=EightBit(16) -> rgb=(0, 0, 0)
19:15:51.078  [bg-tint] ModeUpdate: raw=EightBit(16) -> rgb=(0, 0, 0)
```

## Questions

1. **Is the absence of an initial `ModeUpdate` at startup expected** for (background) plugins? If yes, what is the intended way to obtain the current `Style` / `Palette` / input mode at `load()` time? Is there a synchronous getter or a request I'm missing? (I only found event-driven access via `ModeUpdate`.)
2. Does subscribing to `EventType::ModeUpdate` guarantee an initial delivery of the current mode, or only deltas on change? (Empirically I only see deltas.)
3. **Should `theme_dark "dracula"` be reflected in the plugin-facing `Style`?** I get `text_unselected.background = EightBit(16)` rather than dracula's `#282a36`. Is `theme_dark` only honoured when paired with `theme_light` (or `theme`), and does that pairing requirement also apply to what plugins see in `ModeInfo.style`?
4. Is `style.colors.text_unselected.background` the correct field for "the active theme's terminal/pane background", or is there a more appropriate source for a plugin that wants to derive from the real background colour?

## Workaround currently in use

Reading the background from a plugin config key (a hex string supplied at `load()`), because it is available before any event and does not depend on `ModeUpdate` timing. Works, but it means the plugin can't follow the configured theme automatically - hence this question.
