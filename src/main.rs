/***********************************************************************
 *
 * zellij-background-tint
 *
 * Subtly tint background color of Zellij panes to make each distinct.
 *
 * @author    Marcin Orlowski <mail (#) marcinOrlowski (.) com>
 * @copyright ©2026 Marcin Orlowski
 * @license   http://www.opensource.org/licenses/mit-license.php MIT
 * @link      https://github.com/MarcinOrlowski/zellij-background-tint
 *
 **********************************************************************/

use std::collections::{BTreeMap, HashSet, VecDeque};

use zellij_tile::prelude::*;

// Debug logging. Prepends the plugin's log prefix and gates on the `debug`
// config flag, so both the prefix and the gate live in exactly one place.
// Message arguments are only evaluated when debug logging is enabled.
macro_rules! dbg_log {
    ($self:ident, $fmt:literal $(, $arg:expr)* $(,)?) => {
        if $self.debug {
            eprintln!(concat!("[background-tint] ", $fmt) $(, $arg)*);
        }
    };
}

// Instead of shipping its own palette, this plugin reads the active theme's
// background colour and applies a per-pane tint to it. Magnitudes are chosen
// to be clearly visible on a coloured theme background while still reading as
// a tint of that colour (not a fresh palette):
//   hue        +/- 20 degrees
//   saturation +/- 12 %
//   lightness  +/- 10 %
// Each is overridable from the plugin configuration (`hue`, `saturation`,
// `lightness`), where saturation/lightness are given in percent.
const DEFAULT_MAX_HUE_DEG: f32 = 20.0;
const DEFAULT_MAX_SATURATION_PCT: f32 = 12.0;
const DEFAULT_MAX_LIGHTNESS_PCT: f32 = 10.0;

// How many recently assigned tints to remember when spreading colours apart.
// Each new pane's tint is chosen to be as distant as possible from these, so a
// small cluster of panes does not end up with near-identical shades. Override
// with the `history` config key; 0 disables the spreading (pure per-id hash).
const DEFAULT_HISTORY: usize = 16;
// Candidates generated per pane; the most distant from `recent` wins.
const ASSIGN_ATTEMPTS: u32 = 64;
// RGB Euclidean distance considered "clearly different"; once a candidate beats
// this against every remembered tint we stop early instead of trying all 64.
const SEPARATION_TARGET: f32 = 72.0;

// The event set we subscribe to. Kept in one place so the startup subscribe and
// the permission-grant re-subscribe stay in sync.
const EVENTS: [EventType; 3] = [
    EventType::ModeUpdate,
    EventType::PaneUpdate,
    EventType::PermissionRequestResult,
];

struct BackgroundTint {
    handled: HashSet<PaneId>,
    // The background colour every pane tint is derived from. Either supplied
    // explicitly via the `base` config key (base_locked = true), or read from
    // the active theme on ModeUpdate.
    base_bg: Option<(u8, u8, u8)>,
    // When true, base_bg came from config and ModeUpdate must not overwrite it.
    // Needed because in setups where the terminal background is defined by the
    // emulator (not Zellij), Zellij reports a default black theme background.
    base_locked: bool,
    latest_manifest: Option<PaneManifest>,
    // Ring buffer of recently assigned tints, newest at the back. New tints are
    // picked to sit far from these so nearby panes stay visually distinct.
    recent: VecDeque<(u8, u8, u8)>,
    history: usize,
    debug: bool,
    max_hue_deg: f32,
    max_saturation: f32,
    max_lightness: f32,
}

impl Default for BackgroundTint {
    fn default() -> Self {
        BackgroundTint {
            handled: HashSet::new(),
            base_bg: None,
            base_locked: false,
            latest_manifest: None,
            recent: VecDeque::new(),
            history: DEFAULT_HISTORY,
            debug: false,
            max_hue_deg: DEFAULT_MAX_HUE_DEG,
            max_saturation: DEFAULT_MAX_SATURATION_PCT / 100.0,
            max_lightness: DEFAULT_MAX_LIGHTNESS_PCT / 100.0,
        }
    }
}

register_plugin!(BackgroundTint);

impl ZellijPlugin for BackgroundTint {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.max_hue_deg = parse_f32(&configuration, "hue", DEFAULT_MAX_HUE_DEG).abs();
        self.max_saturation =
            parse_f32(&configuration, "saturation", DEFAULT_MAX_SATURATION_PCT).abs() / 100.0;
        self.max_lightness =
            parse_f32(&configuration, "lightness", DEFAULT_MAX_LIGHTNESS_PCT).abs() / 100.0;
        self.debug = configuration
            .get("debug")
            .map(|v| matches!(v.trim(), "true" | "1" | "yes"))
            .unwrap_or(false);
        self.history = configuration
            .get("history")
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(DEFAULT_HISTORY);

        // Explicit base colour wins. Available immediately at load, so panes can
        // be tinted from the first PaneUpdate without waiting on a ModeUpdate.
        if let Some(rgb) = configuration.get("base").and_then(|s| parse_hex_color(s)) {
            self.base_bg = Some(rgb);
            self.base_locked = true;
        }

        dbg_log!(
            self,
            "load: base={:?} locked={} hue={} sat={} light={}",
            self.base_bg,
            self.base_locked,
            self.max_hue_deg,
            self.max_saturation,
            self.max_lightness,
        );

        // ModeUpdate carries the active theme (fallback base colour). PaneUpdate
        // drives per-pane colouring - it is delivered at startup, so with a
        // config `base` present here panes are tinted immediately, no manual
        // refresh needed. PermissionRequestResult lets us replay work rejected
        // before the grant landed.
        subscribe(&EVENTS);
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
        ]);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::ModeUpdate(mode_info) => {
                let raw = mode_info.style.colors.text_unselected.background;
                let new_base = palette_color_to_rgb(raw);
                dbg_log!(self, "ModeUpdate: raw={:?} -> rgb={:?}", raw, new_base);
                // A config base is authoritative; the theme background is only a
                // fallback (and can be a default-constructed black in setups
                // where the emulator, not Zellij, owns the real background).
                if !self.base_locked && self.base_bg != Some(new_base) {
                    self.base_bg = Some(new_base);
                    self.handled.clear();
                    self.recent.clear();
                    if let Some(manifest) = self.latest_manifest.clone() {
                        self.handle_manifest(manifest);
                    }
                }
            }
            Event::PermissionRequestResult(_) => {
                // First-run grant flow: pre-grant commands were rejected by the
                // host. Re-subscribe and replay the latest manifest so panes get
                // themed as soon as the permission lands.
                subscribe(&EVENTS);
                self.handled.clear();
                self.recent.clear();
                if let Some(manifest) = self.latest_manifest.clone() {
                    self.handle_manifest(manifest);
                }
            }
            Event::PaneUpdate(manifest) => {
                dbg_log!(
                    self,
                    "PaneUpdate: {} terminal pane(s)",
                    manifest.panes.values().flatten().filter(|p| !p.is_plugin).count(),
                );
                self.latest_manifest = Some(manifest.clone());
                self.handle_manifest(manifest);
            }
            _ => {}
        }
        false
    }
}

impl BackgroundTint {
    fn handle_manifest(&mut self, manifest: PaneManifest) {
        // Nothing to tint against until we know the base background.
        let Some(base) = self.base_bg else {
            dbg_log!(self, "handle_manifest: no base yet, skipping");
            return;
        };

        let mut terminals: Vec<_> = manifest
            .panes
            .values()
            .flatten()
            .filter(|pane| !pane.is_plugin)
            .map(|pane| (PaneId::Terminal(pane.id), pane.default_bg.is_some()))
            .collect();

        // Stable ordering makes assignment predictable even though the
        // manifest itself is backed by hash maps.
        terminals.sort_by_key(|(pane_id, _)| *pane_id);

        let live_ids: HashSet<_> = terminals.iter().map(|(pane_id, _)| *pane_id).collect();
        self.handled.retain(|pane_id| live_ids.contains(pane_id));

        for (pane_id, has_existing_background) in terminals {
            if self.handled.contains(&pane_id) {
                continue;
            }

            // Mark first: set_pane_color causes another PaneUpdate, and this
            // prevents that event from recolouring the pane.
            self.handled.insert(pane_id);

            // The API exposes the value but not its provenance. Preserve every
            // existing background, including colours explicitly set by layouts.
            if has_existing_background {
                dbg_log!(self, "{:?}: has existing bg, skipping", pane_id);
                continue;
            }

            let (r, g, b) = self.assign_tint(pane_id, base);
            let background = format!("#{:02x}{:02x}{:02x}", r, g, b);
            dbg_log!(self, "{:?}: set bg {}", pane_id, background);
            set_pane_color(pane_id, None, Some(background));
        }
    }

    // Choose this pane's tinted background. Candidates are deterministic per-pane
    // shifts of the base colour in HSL space (seeded on the pane id, so a pane
    // keeps its tint across updates). To stop nearby panes from landing on
    // near-identical shades - likely when the base is dark/desaturated and the
    // shift box is small - we generate several candidates and keep the one
    // farthest (RGB distance) from the last `history` assigned tints.
    fn assign_tint(&mut self, pane_id: PaneId, base: (u8, u8, u8)) -> (u8, u8, u8) {
        let seed = match pane_id {
            PaneId::Terminal(id) => id as u64,
            PaneId::Plugin(id) => id as u64,
        };

        let attempts = if self.history == 0 { 1 } else { ASSIGN_ATTEMPTS };
        let mut best: (u8, u8, u8) = base;
        let mut best_dist = f32::NEG_INFINITY;

        for attempt in 0..attempts {
            // Mix the attempt index into the seed so each candidate is a
            // different, still-deterministic point in the shift box.
            let h = scramble(seed ^ (attempt as u64).wrapping_mul(0x9E3779B97F4A7C15));

            let hue_shift = symmetric(h & 0xFFFF, self.max_hue_deg);
            let sat_shift = symmetric((h >> 16) & 0xFFFF, self.max_saturation);
            let light_shift = symmetric((h >> 32) & 0xFFFF, self.max_lightness);

            let (mut hue, mut sat, mut light) = rgb_to_hsl(base);
            hue = (hue + hue_shift).rem_euclid(360.0);
            sat = (sat + sat_shift).clamp(0.0, 1.0);
            light = (light + light_shift).clamp(0.0, 1.0);
            let candidate = hsl_to_rgb(hue, sat, light);

            // Distance to the nearest remembered tint; larger is better spread.
            let min_dist = self
                .recent
                .iter()
                .map(|c| rgb_distance(*c, candidate))
                .fold(f32::INFINITY, f32::min);

            if min_dist > best_dist {
                best_dist = min_dist;
                best = candidate;
            }
            // Good enough separation from everything remembered - stop early.
            if min_dist >= SEPARATION_TARGET {
                break;
            }
        }

        if self.history > 0 {
            self.recent.push_back(best);
            while self.recent.len() > self.history {
                self.recent.pop_front();
            }
        }
        best
    }
}

// Euclidean distance between two RGB colours.
fn rgb_distance(a: (u8, u8, u8), b: (u8, u8, u8)) -> f32 {
    let dr = a.0 as f32 - b.0 as f32;
    let dg = a.1 as f32 - b.1 as f32;
    let db = a.2 as f32 - b.2 as f32;
    (dr * dr + dg * dg + db * db).sqrt()
}

// SplitMix64: turns a small, sequential pane id into well-distributed bits so
// adjacent panes get visibly different tints.
fn scramble(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E3779B97F4A7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D049BB133111EB);
    x ^ (x >> 31)
}

// Map 16 bits of hash to a value in [-max, max].
fn symmetric(bits: u64, max: f32) -> f32 {
    let frac = (bits & 0xFFFF) as f32 / 65535.0;
    (frac * 2.0 - 1.0) * max
}

// Parse "#rrggbb" / "rrggbb" (and #rgb / rgb shorthand) into an RGB triple.
fn parse_hex_color(s: &str) -> Option<(u8, u8, u8)> {
    let h = s.trim().trim_start_matches('#');
    match h.len() {
        6 => {
            let r = u8::from_str_radix(&h[0..2], 16).ok()?;
            let g = u8::from_str_radix(&h[2..4], 16).ok()?;
            let b = u8::from_str_radix(&h[4..6], 16).ok()?;
            Some((r, g, b))
        }
        3 => {
            let r = u8::from_str_radix(&h[0..1], 16).ok()?;
            let g = u8::from_str_radix(&h[1..2], 16).ok()?;
            let b = u8::from_str_radix(&h[2..3], 16).ok()?;
            // Expand each nibble: 0xA -> 0xAA.
            Some((r * 17, g * 17, b * 17))
        }
        _ => None,
    }
}

fn parse_f32(config: &BTreeMap<String, String>, key: &str, default: f32) -> f32 {
    config
        .get(key)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

fn palette_color_to_rgb(color: PaletteColor) -> (u8, u8, u8) {
    match color {
        PaletteColor::Rgb(rgb) => rgb,
        // Reuse the SDK's own 256-colour conversion via its rgb string form,
        // so an 8-bit theme background maps exactly as Zellij would render it.
        PaletteColor::EightBit(_) => parse_rgb_str(&color.as_rgb_str()),
    }
}

// Parse Zellij's "rgb(r, g, b)" form into a triple.
fn parse_rgb_str(s: &str) -> (u8, u8, u8) {
    let nums: Vec<u8> = s
        .trim()
        .trim_start_matches("rgb(")
        .trim_end_matches(')')
        .split(',')
        .filter_map(|part| part.trim().parse().ok())
        .collect();
    match nums.as_slice() {
        [r, g, b] => (*r, *g, *b),
        _ => (0, 0, 0),
    }
}

fn rgb_to_hsl((r, g, b): (u8, u8, u8)) -> (f32, f32, f32) {
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let light = (max + min) / 2.0;

    let delta = max - min;
    if delta.abs() < f32::EPSILON {
        return (0.0, 0.0, light); // achromatic
    }

    let sat = if light > 0.5 {
        delta / (2.0 - max - min)
    } else {
        delta / (max + min)
    };

    let hue = if max == r {
        (g - b) / delta + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    };

    (hue * 60.0, sat, light)
}

fn hsl_to_rgb(hue: f32, sat: f32, light: f32) -> (u8, u8, u8) {
    if sat <= 0.0 {
        let v = channel(light);
        return (v, v, v); // achromatic
    }

    let q = if light < 0.5 {
        light * (1.0 + sat)
    } else {
        light + sat - light * sat
    };
    let p = 2.0 * light - q;
    let hk = hue / 360.0;

    (
        channel(hue_to_rgb(p, q, hk + 1.0 / 3.0)),
        channel(hue_to_rgb(p, q, hk)),
        channel(hue_to_rgb(p, q, hk - 1.0 / 3.0)),
    )
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 1.0 / 2.0 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}

fn channel(x: f32) -> u8 {
    (x * 255.0).round().clamp(0.0, 255.0) as u8
}
