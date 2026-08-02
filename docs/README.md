# zellij-background-tint

## The `base` color and the theme-read caveat

When `base` color is set in the config, it will be used as base for tinting. When it is omitted, the
plugin falls back to reading the active theme's background.

**IMPORTANT:** On recent Zellij 0.44.3 obtaining style details is not reliable (this
was [reported](https://github.com/zellij-org/zellij/issues/5408)) and plugin often can read the
color as `#000000` (black), regardless of the style settings. To work that around either set
`base` to the backgrpund color used by your style, or force Zellij to propagate this by enforcing
input-mode change (e.g. <kbd>CTRL</kbd>-<kbd>g</kbd>). This needs to be done only once though.
Unless you switch styles on daily basis, setting up `base` parameter is recommended approach.

---

## Installation

1. Download `zellij-background-tint.wasm` file from project's [Releases](../releases/) page (if artefact
   is provided as `*.zip` file, extract the plugin file from it first.
2. Put it into `~/.config/zellij/plugins/` folder.
3. Back up your current `~/.config/zellij/config.kdl`
4. Edit `config.kdl` and per [Configuration](#configuration) section.
5. Add the alias and startup entry. If you already have either block, add only the indicated line
   inside it:
   ```kdl
   plugins {
       background-tint location="file:/<HOME_DIR>/.config/zellij/plugins/zellij-background-tint.wasm"
   }
   
   load_plugins {
       background-tint
   }
   ```
6. Completely quit all Zellij sessions and start it again.
7. The first launch presents a permission prompt; approve it to enable tinting.

**NOTE:** Use the real absolute path as `<HOME_DIR>` as Zellij plugin URLs do not expand `$HOME` evn
variable.

To uninstall, remove the `background-tint` lines from both `plugins` and `load_plugins`, then remove
the installed `*.wasm` artifact.

---

## Configuration

Default settings shall do the job out of the box, so all the config keys are **optional**.

| key          | unit    | default   | meaning                                                                                 |
|--------------|---------|-----------|-----------------------------------------------------------------------------------------|
| `base`       | #rrggbb | *(theme)* | BG color to tint (i.e. `"#282a36"`). If omitted, the active theme is used.              |
| `hue`        | degrees | `20`      | Max `+/-` hue shift. Set to `0` to disable `hue` mutations.                             |
| `saturation` | percent | `12`      | Max `+/-` saturation shift. Set to `0` to disable saturation mutations.                 |
| `lightness`  | percent | `10`      | Max `+/-` lightness shift. Set to `0` to disable lightness mutations.                   |
| `history`    | count   | `16`      | Buffer to keep track of used colors to ensure their uniqueness. `0` disables spreading. |

### Examples

Minimalistic setup:

```kdl
plugins {
    background-tint location="file:/home/<YOU>/.config/zellij/plugins/zellij-background-tint.wasm" {}
}
```

Complete config entry:

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

---

## Building from sources

Requirements:

- Zellij 0.44.x
- Rust through [rustup](https://rustup.rs/)
- The `wasm32-wasip1` Rust target

### Dev env setup

```bash
# Install compiler manager
sudo apt install -y rustup

# Install + pick stable rustc (apt rustup sets no default)
rustup default stable

# Add wasm target
rustup target add wasm32-wasip1

# build and install (-i) the plugin locally
bin/build.sh -i
```

The `build.sh` script compiles the project. Without arguments it only builds and prints
the path to the produced `.wasm`; with `-i` it also installs the plugin to your local
`${XDG_CONFIG_HOME:-$HOME/.config}/zellij/plugins/` folder.

### Running tests

The unit tests live in `src/tests.rs` and cover the config parsers, the RGB/HSL color
math and the tint assignment logic. They run on your **host** target (not `wasm32-wasip1`),
so no extra target is needed:

```bash
# Run the whole suite
cargo test

# Run a single test (substring match) with its output shown
cargo test assign_tint -- --nocapture
```

The first host build is slow, as `zellij-tile` pulls in `zellij-utils` and its
dependency tree; later runs are incremental.

The same checks CI runs can be reproduced locally:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

### Continuous integration

`.github/workflows/ci.yml` runs on every push and pull request. It has two jobs:

- **Test (host)** - formatting check, Clippy with warnings denied, and the unit tests.
- **Build (wasm32-wasip1)** - runs `bin/build.sh` and uploads the resulting `.wasm`
  as a workflow artifact.
