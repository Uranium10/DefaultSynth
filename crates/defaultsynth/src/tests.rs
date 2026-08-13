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
fn no_parameter_produces_a_non_finite_plain_value() {
    let params = DefaultSynthParams::default();
    for (id, ptr, _) in params.param_map() {
        for normalised in [0.0_f32, 0.5, 1.0] {
            let plain = unsafe { ptr.preview_plain(normalised) };
            assert!(plain.is_finite(), "parameter {id} produced {plain} at {normalised}");
        }
    }
}
