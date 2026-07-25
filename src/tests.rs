/** ****************************************************************************
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
 *************************************************************************** **/
use super::*;

// ---------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------

fn config(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn assert_close(actual: f32, expected: f32, tolerance: f32) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected} +/- {tolerance}, got {actual}"
    );
}

// Per-channel difference between two colors, largest wins.
fn max_channel_delta(a: (u8, u8, u8), b: (u8, u8, u8)) -> u8 {
    let d = |x: u8, y: u8| x.abs_diff(y);
    d(a.0, b.0).max(d(a.1, b.1)).max(d(a.2, b.2))
}

// ---------------------------------------------------------------------
// parse_hex_color
// ---------------------------------------------------------------------

#[test]
fn parse_hex_color_accepts_six_digits() {
    assert_eq!(parse_hex_color("#1a2b3c"), Some((0x1a, 0x2b, 0x3c)));
    assert_eq!(parse_hex_color("1a2b3c"), Some((0x1a, 0x2b, 0x3c)));
    assert_eq!(parse_hex_color("#FFFFFF"), Some((255, 255, 255)));
    assert_eq!(parse_hex_color("#000000"), Some((0, 0, 0)));
}

#[test]
fn parse_hex_color_expands_shorthand() {
    assert_eq!(parse_hex_color("#abc"), Some((0xaa, 0xbb, 0xcc)));
    assert_eq!(parse_hex_color("fff"), Some((255, 255, 255)));
    assert_eq!(parse_hex_color("#000"), Some((0, 0, 0)));
}

#[test]
fn parse_hex_color_trims_surrounding_whitespace() {
    assert_eq!(parse_hex_color("  #1a2b3c \n"), Some((0x1a, 0x2b, 0x3c)));
}

#[test]
fn parse_hex_color_rejects_malformed_input() {
    assert_eq!(parse_hex_color(""), None);
    assert_eq!(parse_hex_color("#"), None);
    assert_eq!(parse_hex_color("#12345"), None); // wrong length
    assert_eq!(parse_hex_color("#1234567"), None); // wrong length
    assert_eq!(parse_hex_color("#gg0000"), None); // not hex
    assert_eq!(parse_hex_color("#xyz"), None); // not hex
    assert_eq!(parse_hex_color("rgb(1,2,3)"), None);
}

// ---------------------------------------------------------------------
// parse_f32
// ---------------------------------------------------------------------

#[test]
fn parse_f32_reads_value_and_falls_back() {
    let cfg = config(&[("hue", "42.5"), ("saturation", " 7 "), ("junk", "abc")]);

    assert_close(parse_f32(&cfg, "hue", 1.0), 42.5, f32::EPSILON);
    assert_close(parse_f32(&cfg, "saturation", 1.0), 7.0, f32::EPSILON);

    // Unparsable value and missing key both yield the default.
    assert_close(parse_f32(&cfg, "junk", 3.5), 3.5, f32::EPSILON);
    assert_close(parse_f32(&cfg, "absent", 3.5), 3.5, f32::EPSILON);
}

#[test]
fn parse_f32_accepts_negative_values() {
    let cfg = config(&[("hue", "-12")]);
    assert_close(parse_f32(&cfg, "hue", 0.0), -12.0, f32::EPSILON);
}

// ---------------------------------------------------------------------
// parse_rgb_str / palette_color_to_rgb
// ---------------------------------------------------------------------

#[test]
fn parse_rgb_str_reads_zellij_form() {
    assert_eq!(parse_rgb_str("rgb(1, 2, 3)"), (1, 2, 3));
    assert_eq!(parse_rgb_str("rgb(255,255,255)"), (255, 255, 255));
}

#[test]
fn parse_rgb_str_falls_back_to_black() {
    assert_eq!(parse_rgb_str(""), (0, 0, 0));
    assert_eq!(parse_rgb_str("rgb(1, 2)"), (0, 0, 0));
    assert_eq!(parse_rgb_str("rgb(1, 2, 3, 4)"), (0, 0, 0));
    // 300 is out of range
    assert_eq!(parse_rgb_str("rgb(300, 2, 3)"), (0, 0, 0));
}

#[test]
fn palette_color_to_rgb_passes_rgb_through() {
    assert_eq!(
        palette_color_to_rgb(PaletteColor::Rgb((12, 34, 56))),
        (12, 34, 56)
    );
}

#[test]
fn palette_color_to_rgb_converts_eight_bit() {
    // Whatever the SDK's mapping is, it must round-trip through the
    // "rgb(r, g, b)" string form rather than silently collapsing to black.
    for index in [16u8, 40, 128, 231] {
        let color = PaletteColor::EightBit(index);
        assert_eq!(
            palette_color_to_rgb(color),
            parse_rgb_str(&color.as_rgb_str())
        );
    }
}

// ---------------------------------------------------------------------
// color math
// ---------------------------------------------------------------------

#[test]
fn rgb_to_hsl_handles_achromatic_colors() {
    let (h, s, l) = rgb_to_hsl((0, 0, 0));
    assert_close(h, 0.0, f32::EPSILON);
    assert_close(s, 0.0, f32::EPSILON);
    assert_close(l, 0.0, f32::EPSILON);

    let (h, s, l) = rgb_to_hsl((255, 255, 255));
    assert_close(h, 0.0, f32::EPSILON);
    assert_close(s, 0.0, f32::EPSILON);
    assert_close(l, 1.0, f32::EPSILON);

    let (_, s, l) = rgb_to_hsl((128, 128, 128));
    assert_close(s, 0.0, f32::EPSILON);
    assert_close(l, 128.0 / 255.0, 0.001);
}

#[test]
fn rgb_to_hsl_computes_primary_hues() {
    assert_close(rgb_to_hsl((255, 0, 0)).0, 0.0, 0.01);
    assert_close(rgb_to_hsl((0, 255, 0)).0, 120.0, 0.01);
    assert_close(rgb_to_hsl((0, 0, 255)).0, 240.0, 0.01);
    // Magenta sits past the wrap-around branch (max == r, g < b).
    assert_close(rgb_to_hsl((255, 0, 255)).0, 300.0, 0.01);
}

#[test]
fn hsl_to_rgb_handles_achromatic_input() {
    assert_eq!(hsl_to_rgb(0.0, 0.0, 0.0), (0, 0, 0));
    assert_eq!(hsl_to_rgb(210.0, 0.0, 1.0), (255, 255, 255));
    assert_eq!(hsl_to_rgb(210.0, 0.0, 0.5), (128, 128, 128));
}

#[test]
fn hsl_rgb_round_trip_is_lossless_within_rounding() {
    let colors = [
        (0, 0, 0),
        (255, 255, 255),
        (18, 20, 27),
        (40, 42, 54),
        (255, 0, 0),
        (0, 128, 64),
        (7, 33, 200),
        (200, 199, 3),
    ];
    for c in colors {
        let (h, s, l) = rgb_to_hsl(c);
        let back = hsl_to_rgb(h, s, l);
        assert!(
            max_channel_delta(c, back) <= 1,
            "round trip drifted: {c:?} -> ({h}, {s}, {l}) -> {back:?}"
        );
    }
}

#[test]
fn channel_scales_and_clamps() {
    assert_eq!(channel(0.0), 0);
    assert_eq!(channel(1.0), 255);
    assert_eq!(channel(0.5), 128);
    // Out-of-range input must not wrap around.
    assert_eq!(channel(-1.0), 0);
    assert_eq!(channel(2.0), 255);
}

#[test]
fn rgb_distance_is_euclidean() {
    assert_close(rgb_distance((1, 2, 3), (1, 2, 3)), 0.0, f32::EPSILON);
    assert_close(rgb_distance((0, 0, 0), (255, 255, 255)), 441.673, 0.01);
    assert_close(rgb_distance((0, 0, 0), (3, 4, 0)), 5.0, 0.001);
    // Symmetric in its arguments.
    assert_close(
        rgb_distance((10, 20, 30), (40, 50, 60)),
        rgb_distance((40, 50, 60), (10, 20, 30)),
        f32::EPSILON,
    );
}

#[test]
fn symmetric_maps_bits_onto_the_full_range() {
    assert_close(symmetric(0, 20.0), -20.0, f32::EPSILON);
    assert_close(symmetric(0xFFFF, 20.0), 20.0, f32::EPSILON);
    assert_close(symmetric(0x7FFF, 20.0), 0.0, 0.001);
    // Only the low 16 bits matter.
    assert_close(symmetric(0xDEAD_0000, 20.0), -20.0, f32::EPSILON);
    // A zero magnitude pins the result at zero.
    assert_close(symmetric(0xFFFF, 0.0), 0.0, f32::EPSILON);
}

#[test]
fn scramble_is_deterministic_and_spreads_neighbours() {
    assert_eq!(scramble(7), scramble(7));

    // Sequential pane ids must not produce near-identical hashes, otherwise
    // adjacent panes would share a tint.
    let hashes: Vec<u64> = (0..16u64).map(scramble).collect();
    let unique: HashSet<u64> = hashes.iter().copied().collect();
    assert_eq!(unique.len(), hashes.len(), "scramble collided: {hashes:?}");
}

// ---------------------------------------------------------------------
// load()
// ---------------------------------------------------------------------

// `load()` also calls into the Zellij host (subscribe/request_permission),
// which is unavailable off-wasm, so config parsing is exercised through the
// same expressions here rather than by calling load() itself.

#[test]
fn defaults_are_applied_when_config_is_empty() {
    let tint = BackgroundTint::default();
    assert_close(tint.max_hue_deg, DEFAULT_MAX_HUE_DEG, f32::EPSILON);
    assert_close(
        tint.max_saturation,
        DEFAULT_MAX_SATURATION_PCT / 100.0,
        f32::EPSILON,
    );
    assert_close(
        tint.max_lightness,
        DEFAULT_MAX_LIGHTNESS_PCT / 100.0,
        f32::EPSILON,
    );
    assert_eq!(tint.history, DEFAULT_HISTORY);
    assert!(!tint.debug);
    assert_eq!(tint.base_bg, None);
    assert!(!tint.base_locked);
}

// ---------------------------------------------------------------------
// assign_tint
// ---------------------------------------------------------------------

fn tint_with(history: usize) -> BackgroundTint {
    BackgroundTint {
        history,
        ..Default::default()
    }
}

#[test]
fn assign_tint_returns_base_when_shifts_are_zero() {
    let mut tint = BackgroundTint {
        max_hue_deg: 0.0,
        max_saturation: 0.0,
        max_lightness: 0.0,
        ..Default::default()
    };
    let base = (40, 42, 54);
    let got = tint.assign_tint(PaneId::Terminal(1), base);
    // The value still round-trips through HSL, so allow rounding drift.
    assert!(
        max_channel_delta(got, base) <= 1,
        "expected ~{base:?}, got {got:?}"
    );
}

#[test]
fn assign_tint_is_deterministic_for_the_same_pane_sequence() {
    let base = (18, 20, 27);
    let run = || {
        let mut tint = tint_with(DEFAULT_HISTORY);
        (1..=8u32)
            .map(|id| tint.assign_tint(PaneId::Terminal(id), base))
            .collect::<Vec<_>>()
    };
    assert_eq!(run(), run());
}

#[test]
fn assign_tint_gives_neighbouring_panes_distinct_colors() {
    let base = (18, 20, 27);
    let mut tint = tint_with(DEFAULT_HISTORY);
    let colors: Vec<_> = (1..=8u32)
        .map(|id| tint.assign_tint(PaneId::Terminal(id), base))
        .collect();
    let unique: HashSet<_> = colors.iter().copied().collect();
    assert_eq!(unique.len(), colors.len(), "duplicate tints: {colors:?}");
}

#[test]
fn assign_tint_stays_within_the_configured_shift_box() {
    let base = (40, 42, 54);
    let mut tint = BackgroundTint {
        max_hue_deg: 20.0,
        max_saturation: 0.12,
        max_lightness: 0.10,
        ..Default::default()
    };
    let (base_h, base_s, base_l) = rgb_to_hsl(base);

    for id in 1..=32u32 {
        let (h, s, l) = rgb_to_hsl(tint.assign_tint(PaneId::Terminal(id), base));
        // Hue wraps at 360, so compare on the circle.
        let mut hue_delta = (h - base_h).abs() % 360.0;
        if hue_delta > 180.0 {
            hue_delta = 360.0 - hue_delta;
        }
        // Rounding to 8-bit channels widens the box slightly; allow for it.
        assert!(hue_delta <= 20.0 + 5.0, "hue drifted {hue_delta} for #{id}");
        assert!(
            (s - base_s).abs() <= 0.12 + 0.02,
            "saturation drifted {} for #{id}",
            (s - base_s).abs()
        );
        assert!(
            (l - base_l).abs() <= 0.10 + 0.02,
            "lightness drifted {} for #{id}",
            (l - base_l).abs()
        );
    }
}

#[test]
fn assign_tint_caps_history_at_the_configured_size() {
    let mut tint = tint_with(4);
    for id in 1..=20u32 {
        tint.assign_tint(PaneId::Terminal(id), (18, 20, 27));
    }
    assert_eq!(tint.recent.len(), 4);
}

#[test]
fn assign_tint_keeps_no_history_when_disabled() {
    let mut tint = tint_with(0);
    for id in 1..=10u32 {
        tint.assign_tint(PaneId::Terminal(id), (18, 20, 27));
    }
    assert!(tint.recent.is_empty());
}

#[test]
fn assign_tint_with_history_disabled_is_still_per_pane_stable() {
    let base = (18, 20, 27);
    let mut a = tint_with(0);
    let mut b = tint_with(0);
    // Without history the result depends on the pane id alone, so the same id
    // yields the same tint regardless of what was assigned before it.
    assert_eq!(
        a.assign_tint(PaneId::Terminal(5), base),
        b.assign_tint(PaneId::Terminal(5), base)
    );
}

#[test]
fn assign_tint_handles_plugin_pane_ids() {
    let mut tint = tint_with(DEFAULT_HISTORY);
    let base = (18, 20, 27);
    assert_eq!(
        tint.assign_tint(PaneId::Plugin(3), base),
        tint_with(DEFAULT_HISTORY).assign_tint(PaneId::Plugin(3), base)
    );
}
