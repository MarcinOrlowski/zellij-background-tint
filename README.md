# zellij-background-tint

A handy headless Zellij plugin that gives every terminal pane you create a subtly different
background **derived from your existing theme**!

It reads the active theme's background color and applies a small, per-pane shift to it (hue,
saturation, lightness) so pane boundaries become apparent without replacing your carefully chosen
colors.

## What it does

- Reads the theme background from the active Zellij style (`ModeUpdate`).
- Tints terminal panes that exist when the plugin starts.
- Tints tiled, floating, stacked, shell, and command panes created later.
- Leaves Zellij UI plugins and its own background plugin pane untouched.
- Preserves panes that already have a `default_bg`, including layout colors.
- Assigns each pane once, so focus, resize, rename, and command updates do not recolor it.
- Keeps track of used colors to ensure no two panes are identical.

The plugin requests only `ReadApplicationState` and `ChangeApplicationState`.

## How the tint is computed

1. A **base** background color is chosen (see `base` below), converted to HSL.
2. The pane id is hashed (SplitMix64) into three independent shifts:

- hue        `+/- 20°`
- saturation `+/- 12 %`
- lightness  `+/- 10 %`

3. Several such candidates are generated for the pane; the one **farthest** (in RGB distance) from
   the last `history` assigned tints is kept. This keeps a cluster of panes visually distinct
   instead of letting random shifts collide on near-identical shades - which is easy to hit when the
   base is dark and desaturated and the shift box is small.
4. The chosen color is converted to `#rrggbb` and set on the pane.

Because the candidates are seeded on the pane id rather than pure random, a pane keeps its tint
across updates while adjacent panes are pushed apart.

## Configuration

All keys are **optional**.

| key          | unit    | default   | meaning                                                                            |
|--------------|---------|-----------|------------------------------------------------------------------------------------|
| `base`       | #rrggbb | *(theme)* | Background color to tint (i.e. `"#282a36"`). If omitted, the active theme is used. |
| `hue`        | degrees | `20`      | Max `+/-` hue shift. Set to `0` to disable `hue` mutations.                        |
| `saturation` | percent | `12`      | Max `+/-` saturation shift. Set to `0` to disable saturation mutations.            |
| `lightness`  | percent | `10`      | Max `+/-` lightness shift. Set to `0` to disable lightness mutations.              |
| `history`    | count   | `16`      | How many recent tints to spread new panes away from. `0` disables spreading.       |

### Examples

Minimalistic base:

```kdl
plugins {
    background-tint location="file:/home/<YOU>/.config/zellij/plugins/zellij-background-tint.wasm" {}
}
```

Complete setup:

```kdl
plugins {
    background-tint location="file:/home/<YOU>/.config/zellij/plugins/zellij-background-tint.wasm" {
        base       "#282a36"
        hue        "20"
        saturation "12"
        lightness  "10"
    }
}
```

### The `base` color and the theme-read caveat

When `base` color is set, it is available at plugin load, so panes are tinted immediately and
deterministically - this is the recommended setup.

When `base` is omitted, the plugin falls back to reading the active theme's background from
`ModeUpdate`. But on recent Zellij 0.44.3 this is unreliable as no `ModeUpdate` is delivered at
startup (only after the **first** input-mode change, e.g. <kbd>CTRL</kbd>-<kbd>g</kbd>`), and the
reported background may not reflect the configured theme.

This was reported in this ticket https://github.com/zellij-org/zellij/issues/5408

## Installation

Requirements:

- Zellij 0.44.x
- Rust through [rustup](https://rustup.rs/)
- The `wasm32-wasip1` Rust target

```sh
rustup target add wasm32-wasip1
./bin/install.sh
```

This installs the plugin to:

```text
${XDG_CONFIG_HOME:-$HOME/.config}/zellij/plugins/zellij-background-tint.wasm
```

Back up `~/.config/zellij/config.kdl`, then add the alias and startup entry. If you already have
either block, add only the indicated line inside it:

```kdl
plugins {
    background-tint location="file:/home/YOU/.config/zellij/plugins/zellij-background-tint.wasm"
}

load_plugins {
    background-tint
}
```

Use the real absolute path - Zellij plugin URLs do not expand `$HOME`. Completely quit all Zellij
sessions and start Zellij again. The first launch presents a permission prompt; approve it to enable
tinting.

## Add it to an existing session

```sh
zellij action start-or-reload-plugin \
  "file:${XDG_CONFIG_HOME:-$HOME/.config}/zellij/plugins/zellij-background-tint.wasm"
```

A complete Zellij restart is the reliable way to activate a newly installed build, since Zellij 0.44
can reuse a cached WebAssembly module at the same URL.

## Disable, reset, and uninstall

Remove or comment out `background-tint` inside `load_plugins`, then restart Zellij. Existing colors
remain until their panes close. Reset the current pane immediately with:

```sh
zellij action set-pane-color --reset
```

To uninstall, remove the `background-tint` lines from both `plugins` and
`load_plugins`, then remove the installed artifact:

```sh
rm -f "${XDG_CONFIG_HOME:-$HOME/.config}/zellij/plugins/zellij-background-tint.wasm"
```

## Design notes and limitations

- The theme background is captured from the **first** `ModeUpdate`. A live theme switch during a
  session does not re-tint already-open panes; new panes pick up the new theme. Restart the plugin
  to re-base after a theme change.
- Zellij 0.44 exposes a pane's `default_bg` value but not its provenance. This plugin therefore
  preserves every non-empty background. That reliably protects layout-defined and resurrected
  colors, but also intentionally leaves alone a pane colored by another source.
- The "theme background" is read from `text_unselected.background`, which is what Zellij maps to
  `Palette.bg`.

## Installation

````bash
# Install compiler manager
sudo apt install -y rustup

# Install + pick stable rustc (apt rustup sets no default)
rustup default stable

# Add wasm target
rustup target add wasm32-wasip1
```

## License

[MIT](LICENSE). Derived from
[zellij-pane-colors](https://github.com/sudo-vaibhav/zellij-pane-colors)
© 2026 Vaibhav Chopra. Modifications © 2026 Marcin Orlowski.
