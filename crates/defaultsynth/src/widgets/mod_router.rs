//! The one place a modulation routing is written.
//!
//! Dragging a ✛ handle onto a knob has to end up in the matrix, and the matrix
//! is eight slots of three parameters. Rather than give every drop target its own
//! set of twenty-four parameter bindings, one invisible view owns them all and
//! everything else just says what it wants routed where.
//!
//! It also publishes which destinations currently have something pointed at
//! them, which is what lights the rings next to the knobs.

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use std::sync::Arc;

use crate::params::{DefaultSynthParams, ModDestSlotParam, ModSourceSlotParam};

/// Depth a fresh routing is given.
///
/// Not zero. A row that exists but does nothing looks like a bug from the
/// outside, and the amount is the easiest thing to turn back down.
const DEFAULT_AMOUNT: f32 = 0.5;

/// Ask the router to point `source` at `destination`.
pub enum ModRouteEvent {
    Assign { source: ModSourceSlotParam, destination: ModDestSlotParam },
    /// Clears every slot aimed at a destination.
    Clear(ModDestSlotParam),
}

/// What the router reports back once it has written a routing.
pub enum ModRoutingChanged {
    /// One bit per [`ModDestSlotParam`] index that has something routed to it.
    Mask(u32),
}

/// The routed-destination mask read straight off the parameters.
///
/// Used once, to seed the editor before the router has had anything to do: a
/// patch loaded before the window opened already has routings, and their rings
/// should be lit the moment it appears.
pub fn routed_mask_of(params: &DefaultSynthParams) -> u32 {
    let mut mask = 0;
    for slot in &params.matrix {
        if slot.source.value() != ModSourceSlotParam::None
            && slot.destination.value() != ModDestSlotParam::None
        {
            mask |= 1 << slot.destination.value().to_index();
        }
    }
    mask
}

struct Slot {
    source: ParamWidgetBase,
    destination: ParamWidgetBase,
    amount: ParamWidgetBase,
}

pub struct ModRouter {
    slots: Vec<Slot>,
}

impl ModRouter {
    /// Builds the router. It draws nothing; give it no size in the layout.
    pub fn new<L>(cx: &mut Context, params: L) -> Handle<'_, Self>
    where
        L: Lens<Target = Arc<DefaultSynthParams>> + Clone,
    {
        let slots = (0..ds_dsp::MOD_SLOTS)
            .map(|index| Slot {
                source: ParamWidgetBase::new(cx, params.clone(), move |p| &p.matrix[index].source),
                destination: ParamWidgetBase::new(cx, params.clone(), move |p| &p.matrix[index].destination),
                amount: ParamWidgetBase::new(cx, params.clone(), move |p| &p.matrix[index].amount),
            })
            .collect();
        Self { slots }.build(cx, |_| {})
    }

    /// Current source and destination of one slot.
    fn slot_route(&self, index: usize) -> (ModSourceSlotParam, ModDestSlotParam) {
        let slot = &self.slots[index];
        (
            ModSourceSlotParam::from_index(step_index(&slot.source)),
            ModDestSlotParam::from_index(step_index(&slot.destination)),
        )
    }

    /// One bit per destination that something is currently routed to.
    pub fn routed_mask(&self) -> u32 {
        let mut mask = 0;
        for index in 0..self.slots.len() {
            let (source, destination) = self.slot_route(index);
            if source != ModSourceSlotParam::None && destination != ModDestSlotParam::None {
                mask |= 1 << destination.to_index();
            }
        }
        mask
    }

    /// Points `source` at `destination`, reusing a slot if one already matches.
    ///
    /// Dropping the same source on the same knob twice is a mistake, not a
    /// request for two rows, so the second drop lands on the row that is already
    /// there and leaves its amount alone.
    fn assign(&self, cx: &mut EventContext, source: ModSourceSlotParam, destination: ModDestSlotParam) {
        if source == ModSourceSlotParam::None || destination == ModDestSlotParam::None {
            return;
        }
        if (0..self.slots.len()).any(|index| self.slot_route(index) == (source, destination)) {
            return;
        }
        let Some(free) = (0..self.slots.len())
            .find(|index| self.slot_route(*index) == (ModSourceSlotParam::None, ModDestSlotParam::None))
        else {
            // Every row is taken. Silently dropping the gesture is better than
            // overwriting a routing the player set up on purpose.
            nih_debug_assert_failure!("the modulation matrix is full");
            return;
        };

        let slot = &self.slots[free];
        set_step(cx, &slot.source, source.to_index());
        set_step(cx, &slot.destination, destination.to_index());
        set_normalized(cx, &slot.amount, amount_to_normalized(DEFAULT_AMOUNT));
    }

    /// Empties every slot pointed at a destination.
    fn clear(&self, cx: &mut EventContext, destination: ModDestSlotParam) {
        if destination == ModDestSlotParam::None {
            return;
        }
        for index in 0..self.slots.len() {
            if self.slot_route(index).1 != destination {
                continue;
            }
            let slot = &self.slots[index];
            set_step(cx, &slot.source, ModSourceSlotParam::None.to_index());
            set_step(cx, &slot.destination, ModDestSlotParam::None.to_index());
            set_normalized(cx, &slot.amount, amount_to_normalized(0.0));
        }
    }
}

/// Which discrete step a parameter is currently on.
fn step_index(param: &ParamWidgetBase) -> usize {
    let steps = param.step_count().unwrap_or(0).max(1) as f32;
    (param.unmodulated_normalized_value() * steps).round() as usize
}

fn set_step(cx: &mut EventContext, param: &ParamWidgetBase, index: usize) {
    let steps = param.step_count().unwrap_or(0).max(1) as f32;
    set_normalized(cx, param, index as f32 / steps);
}

fn set_normalized(cx: &mut EventContext, param: &ParamWidgetBase, normalized: f32) {
    param.begin_set_parameter(cx);
    param.set_normalized_value(cx, normalized);
    param.end_set_parameter(cx);
}

/// The amount runs -1..1, so a normalised 0.5 is no modulation at all.
fn amount_to_normalized(amount: f32) -> f32 {
    (amount.clamp(-1.0, 1.0) + 1.0) * 0.5
}

impl View for ModRouter {
    fn element(&self) -> Option<&'static str> {
        Some("mod-router")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|route_event, meta| {
            match route_event {
                ModRouteEvent::Assign { source, destination } => self.assign(cx, *source, *destination),
                ModRouteEvent::Clear(destination) => self.clear(cx, *destination),
            }
            // The rings next to the knobs read this rather than each watching
            // all eight slots for themselves.
            cx.emit(ModRoutingChanged::Mask(self.routed_mask()));
            meta.consume();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zero_amount_sits_at_the_middle_of_the_range() {
        // The matrix amount is bipolar, so "no modulation" is the centre of the
        // normalised range rather than the bottom of it.
        assert!((amount_to_normalized(0.0) - 0.5).abs() < 1e-6);
        assert!((amount_to_normalized(-1.0) - 0.0).abs() < 1e-6);
        assert!((amount_to_normalized(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_new_routing_is_audible_but_not_extreme() {
        // Half depth: enough that dropping an LFO on a knob obviously did
        // something, not so much that it swamps the patch.
        assert!(DEFAULT_AMOUNT > 0.0 && DEFAULT_AMOUNT < 1.0);
    }

    #[test]
    fn every_destination_fits_in_the_routed_mask() {
        // The mask is one u32 bit per destination, so the enum has to stay
        // shorter than that.
        assert!(ModDestSlotParam::variants().len() <= 32);
    }
}
