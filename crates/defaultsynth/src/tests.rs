//! Parameter-tree invariants.
//!
//! These guard the things a plugin host relies on and that `clap-validator`
//! checks at runtime: unique stable IDs, and string conversions that round-trip.

use crate::params::DefaultSynthParams;
use nih_plug::prelude::*;
use std::collections::HashMap;

fn param_ids() -> Vec<String> {
    DefaultSynthParams::default()
        .param_map()
        .into_iter()
        .map(|(id, _, _)| id)
        .collect()
}

#[test]
fn every_parameter_id_is_unique() {
    // A duplicate ID silently makes one parameter shadow another, which shows up
    // as state failing to restore rather than as a build error.
    let ids = param_ids();
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for id in &ids {
        *seen.entry(id.as_str()).or_default() += 1;
    }
    let duplicates: Vec<_> = seen.iter().filter(|(_, count)| **count > 1).collect();
    assert!(duplicates.is_empty(), "duplicate parameter ids: {duplicates:?}");
}

#[test]
fn exposes_every_panel_group() {
    let ids = param_ids();
    for prefix in ["oa_", "ob_", "oc_", "noise_", "fa_", "fb_", "aeg_", "feg_", "voc_"] {
        assert!(
            ids.iter().any(|id| id.starts_with(prefix)),
            "no parameters were exported for prefix {prefix}"
        );
    }
    assert!(ids.iter().any(|id| id == "master"), "master gain was not exported");
}

#[test]
fn string_conversions_round_trip() {
    // clap-validator rejects a plugin whose value->string->value conversion does
    // not land back on the same value; a doubled unit suffix is the usual cause.
    let params = DefaultSynthParams::default();
    for (id, ptr, _) in params.param_map() {
        for normalised in [0.0_f32, 0.25, 0.5, 0.75, 1.0] {
            let text = unsafe { ptr.normalized_value_to_string(normalised, false) };
            let parsed = unsafe { ptr.string_to_normalized_value(&text) };
            let Some(parsed) = parsed else {
                panic!("parameter {id} could not parse back its own output {text:?}");
            };
            let round_tripped = unsafe { ptr.normalized_value_to_string(parsed, false) };
            assert_eq!(
                text, round_tripped,
                "parameter {id} did not round-trip: {normalised} -> {text:?} -> {parsed} -> {round_tripped:?}"
            );
        }
    }
}

#[test]
fn sync_rates_land_on_the_expected_musical_lengths() {
    use crate::params::SyncRateParam;

    // At 120 bpm a whole note is two seconds, so a bar is two seconds too.
    let bpm = 120.0;
    assert!((SyncRateParam::OneBar.cycle_seconds(bpm, false, false) - 2.0).abs() < 1e-5);
    assert!((SyncRateParam::FourBar.cycle_seconds(bpm, false, false) - 8.0).abs() < 1e-5);
    assert!((SyncRateParam::OneOver4.cycle_seconds(bpm, false, false) - 0.5).abs() < 1e-5);
    assert!((SyncRateParam::OneOver128.cycle_seconds(bpm, false, false) - 2.0 / 128.0).abs() < 1e-6);
}

#[test]
fn triplets_shorten_and_dots_lengthen_the_cycle() {
    use crate::params::SyncRateParam;

    let plain = SyncRateParam::OneOver4.cycle_seconds(120.0, false, false);
    let triplet = SyncRateParam::OneOver4.cycle_seconds(120.0, true, false);
    let dotted = SyncRateParam::OneOver4.cycle_seconds(120.0, false, true);
    // Three triplets fill two plain notes; a dotted note is half again as long.
    assert!((triplet - plain * 2.0 / 3.0).abs() < 1e-6, "triplet was {triplet}");
    assert!((dotted - plain * 1.5).abs() < 1e-6, "dotted was {dotted}");
    // Both set at once is not a meaningful combination, so triplet wins.
    assert!((SyncRateParam::OneOver4.cycle_seconds(120.0, true, true) - triplet).abs() < 1e-6);
}

#[test]
fn sync_rates_are_ordered_from_fastest_to_slowest() {
    use crate::params::SyncRateParam;

    // The knob steps through these in order, so the list has to be monotonic or
    // turning it right would sometimes speed the LFO up and sometimes slow it.
    let rates = [
        SyncRateParam::OneOver128, SyncRateParam::OneOver64, SyncRateParam::OneOver32,
        SyncRateParam::OneOver16, SyncRateParam::OneOver8, SyncRateParam::OneOver4,
        SyncRateParam::OneOver2, SyncRateParam::OneBar, SyncRateParam::TwoBar, SyncRateParam::FourBar,
    ];
    for pair in rates.windows(2) {
        assert!(
            pair[0].cycle_in_whole_notes() < pair[1].cycle_in_whole_notes(),
            "{:?} should be shorter than {:?}",
            pair[0],
            pair[1]
        );
    }
}

#[test]
fn filter_routing_defaults_to_an_even_split() {
    let params = DefaultSynthParams::default();
    // Centre is 50:50 between the two filters, which is what the design's A-B
    // slider shows at rest.
    assert!((params.osc_a.filter_send.value() - 0.5).abs() < 1e-6);
    assert_eq!(params.osc_a.filter_send.to_string(), "50:50");
}

#[test]
fn no_parameter_produces_a_non_finite_plain_value() {
    let params = DefaultSynthParams::default();
    for (id, ptr, _) in params.param_map() {
        for normalised in [0.0_f32, 0.5, 1.0] {
            let plain = unsafe { ptr.preview_plain(normalised) };
            assert!(plain.is_finite(), "parameter {id} produced {plain} at {normalised}");
        }
    }
}

#[test]
fn matrix_enum_indices_match_the_dsp_ones() {
    use crate::params::{LfoShapeParam, LfoTriggerParam, ModDestSlotParam, ModSourceSlotParam};

    // The plugin enums and the DSP ones are paired by position, not by name, so
    // inserting a variant in one and not the other would silently reroute every
    // slot below it. This is the test that catches that.
    for index in 0..ModSourceSlotParam::variants().len() {
        assert_eq!(
            ModSourceSlotParam::from_index(index).to_dsp(),
            ds_dsp::ModSource::from_index(index),
            "source index {index} disagrees"
        );
    }
    for index in 0..ModDestSlotParam::variants().len() {
        assert_eq!(
            ModDestSlotParam::from_index(index).to_dsp(),
            ds_dsp::ModDest::from_index(index),
            "destination index {index} disagrees"
        );
    }
    // Every variant must be reachable: a matrix row that cannot select a
    // destination is a row the player cannot use.
    assert_eq!(ModSourceSlotParam::variants().len(), 11);
    assert_eq!(ModDestSlotParam::variants().len(), 14);
    // Six built-in shapes plus the drawn one.
    assert_eq!(LfoShapeParam::variants().len(), 7);
    for index in 0..LfoShapeParam::variants().len() {
        assert_eq!(
            LfoShapeParam::from_index(index).to_dsp(),
            ds_dsp::LfoShape::from_index(index),
            "shape index {index} disagrees"
        );
    }
    assert_eq!(LfoTriggerParam::variants().len(), 3);
}

#[test]
fn every_matrix_slot_starts_disconnected() {
    let params = DefaultSynthParams::default();
    // A synth that modulates something the moment it loads would be a surprise,
    // and the amounts are what the MATRIX page will edit later.
    for (index, slot) in params.matrix.iter().enumerate() {
        assert_eq!(slot.to_dsp().source, ds_dsp::ModSource::None, "slot {index} starts routed");
        assert_eq!(slot.to_dsp().destination, ds_dsp::ModDest::None, "slot {index} starts routed");
    }
}

#[test]
fn each_lfo_keeps_its_own_drawn_curve() {
    use ds_dsp::{CurvePoint, LfoCurve};

    let params = DefaultSynthParams::default();
    // Persisted fields land in one flat map, so a shared key would have the four
    // LFOs silently overwriting each other. Give each a distinguishable shape.
    for (index, curve) in [&params.lfo1_curve, &params.lfo2_curve, &params.lfo3_curve, &params.lfo4_curve]
        .into_iter()
        .enumerate()
    {
        let height = (index + 1) as f32 / 8.0;
        curve.store(LfoCurve::from_points(&[
            CurvePoint::new(0.0, 0.0),
            CurvePoint::new(0.5, height),
            CurvePoint::new(1.0, 0.0),
        ]));
    }

    let fields = params.serialize_fields();
    for key in ["lfo1curve", "lfo2curve", "lfo3curve", "lfo4curve"] {
        assert!(fields.contains_key(key), "{key} was not persisted");
    }

    let restored = DefaultSynthParams::default();
    for (key, value) in &fields {
        restored.deserialize_fields(&std::collections::BTreeMap::from([(key.clone(), value.clone())]));
    }
    for (index, curve) in
        [&restored.lfo1_curve, &restored.lfo2_curve, &restored.lfo3_curve, &restored.lfo4_curve]
            .into_iter()
            .enumerate()
    {
        let expected = (index + 1) as f32 / 8.0;
        let peak = curve.load().sample(0.5);
        assert!((peak - expected).abs() < 1e-5, "LFO {} came back as {peak}, wanted {expected}", index + 1);
    }
}

#[test]
fn a_drawn_curve_survives_being_saved_and_loaded() {
    use ds_dsp::{CurvePoint, LfoCurve};

    let params = DefaultSynthParams::default();
    let drawn = LfoCurve::from_points(&[
        CurvePoint::new(0.0, 0.25),
        CurvePoint { x: 0.2, y: 1.0, tension: 0.7 },
        CurvePoint { x: 0.6, y: 0.1, tension: -0.4 },
        CurvePoint::new(1.0, 0.25),
    ]);
    params.lfo1_curve.store(drawn);

    let restored = DefaultSynthParams::default();
    restored.deserialize_fields(&params.serialize_fields());

    let loaded = restored.lfo1_curve.load();
    assert_eq!(loaded.len(), drawn.len(), "point count changed");
    for step in 0..=100 {
        let phase = step as f32 / 100.0;
        assert!(
            (loaded.sample(phase) - drawn.sample(phase)).abs() < 1e-5,
            "curve changed at {phase}: {} vs {}",
            loaded.sample(phase),
            drawn.sample(phase)
        );
    }
}
