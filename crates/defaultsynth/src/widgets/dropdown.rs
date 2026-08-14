//! Dropdown for the design's long grey selector boxes.
//!
//! The waveform, modulation source, noise colour, filter mode and LFO trigger
//! all pick one of a handful of named values, which reads as a dropdown rather
//! than as something you scrub. The entries come from the parameter itself:
//! a discrete parameter reports how many steps it has, and each step formats
//! itself, so a new enum variant appears here without any extra wiring.
//!
//! The list is built here rather than with VIZIA's `Dropdown`. Its `Popup` keeps
//! itself on screen by nudging its own translation from inside its geometry
//! handler, and when the list cannot fit below the box that nudge alternates
//! between two positions, each one triggering the next layout pass. The editor
//! locks up. Dropdowns near the bottom of the window — the filter mode ones —
//! hit it every time. Placement is decided once here, when the list opens.

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;

/// Row height in the open list.
///
/// Lives here rather than only in the stylesheet because the widget has to know
/// how tall the list will be *before* it is built, to decide whether it fits
/// below the box. The `.dropdown-item` rule is set from this value.
const ITEM_HEIGHT: f32 = 22.0;
/// Inset and row gap of the list, matching `.dropdown-list`.
const LIST_PADDING: f32 = 3.0;
const LIST_GAP: f32 = 1.0;
/// Gap between the box and the list it opens.
const LIST_OFFSET: f32 = 4.0;

/// Normalised positions and captions for every step of a discrete parameter.
fn entries(param: &ParamWidgetBase) -> Vec<(f32, String)> {
    // `step_count` is one less than the number of values, matching the
    // normalised range's endpoints.
    let Some(steps) = param.step_count() else { return Vec::new() };
    (0..=steps)
        .map(|index| {
            let normalized = index as f32 / steps.max(1) as f32;
            (normalized, param.normalized_value_to_string(normalized, false))
        })
        .collect()
}

/// Height the list needs for `count` entries.
fn list_height(count: usize) -> f32 {
    let gaps = count.saturating_sub(1) as f32 * LIST_GAP;
    count as f32 * ITEM_HEIGHT + gaps + LIST_PADDING * 2.0
}

/// Whether the list is open, and where it sits relative to the box.
///
/// One value rather than two so a single binding rebuilds the list already in
/// the right place, instead of building it and then moving it.
#[derive(Debug, Clone, Copy, PartialEq, Default, Data)]
struct Placement {
    open: bool,
    /// Offset of the list's top edge from the box's top edge, in pixels.
    top: f32,
}

#[derive(Lens)]
struct DropdownState {
    placement: Placement,
}

/// Mirrors the view's own state so the list has a lens to bind to.
enum PlacementEvent {
    Set(Placement),
}

impl Model for DropdownState {
    fn event(&mut self, _cx: &mut EventContext, event: &mut Event) {
        event.map(|placement_event, meta| {
            let PlacementEvent::Set(placement) = placement_event;
            self.placement = *placement;
            meta.consume();
        });
    }
}

pub struct ParamDropdown {
    param_base: ParamWidgetBase,
    /// How many entries the list has, which is what decides its height.
    count: usize,
    placement: Placement,
}

impl ParamDropdown {
    pub fn new<L, Params, P, FMap>(cx: &mut Context, params: L, params_to_param: FMap) -> Handle<'_, Self>
    where
        L: Lens<Target = Params> + Clone,
        Params: 'static,
        P: Param + 'static,
        FMap: Fn(&Params) -> &P + Copy + 'static,
    {
        let count = ParamWidgetBase::new(cx, params.clone(), params_to_param)
            .step_count()
            .map_or(0, |steps| steps + 1);

        Self { param_base: ParamWidgetBase::new(cx, params.clone(), params_to_param), count, placement: Placement::default() }
            .build(
                cx,
                ParamWidgetBase::build_view(params.clone(), params_to_param, move |cx, param_data| {
                    let options = entries(&ParamWidgetBase::new(cx, params, params_to_param));

                    DropdownState { placement: Placement::default() }.build(cx);

                    // A click anywhere else shuts the list. `is_over` is true for
                    // the box and everything inside it, the open list included,
                    // so this does not fight the list's own presses.
                    cx.add_listener(move |dropdown: &mut ParamDropdown, cx, event| {
                        event.map(|window_event, _| {
                            if matches!(window_event, WindowEvent::MouseDown(_))
                                && dropdown.placement.open
                                && !cx.is_over()
                            {
                                dropdown.placement = Placement::default();
                                cx.emit(PlacementEvent::Set(dropdown.placement));
                            }
                        });
                    });

                    // Closed state: the current value, formatted by the parameter.
                    Label::new(
                        cx,
                        param_data.make_lens(|param| {
                            param.normalized_value_to_string(param.unmodulated_normalized_value(), false)
                        }),
                    )
                    .class("dropdown-value");

                    Binding::new(cx, DropdownState::placement, move |cx, placement| {
                        let placement = placement.get(cx);
                        if !placement.open {
                            return;
                        }
                        VStack::new(cx, |cx| {
                            for (normalized, caption) in &options {
                                let normalized = *normalized;
                                Label::new(cx, caption.as_str())
                                    .class("dropdown-item")
                                    .height(Pixels(ITEM_HEIGHT))
                                    .on_press(move |cx| {
                                        cx.emit(ParamDropdownEvent::Select(normalized));
                                        cx.emit(PlacementEvent::Set(Placement::default()));
                                    });
                            }
                        })
                        .class("dropdown-list")
                        .position_type(PositionType::SelfDirected)
                        .top(Pixels(placement.top))
                        .left(Pixels(0.0))
                        .z_index(100);
                    });
                }),
            )
    }

    /// Decides where the list goes, opening upward when it would not fit below.
    fn placement_for(&self, cx: &EventContext) -> Placement {
        let bounds = cx.bounds();
        let window = cx.cache.get_bounds(Entity::root());
        let needed = list_height(self.count);
        let fits_below = bounds.bottom() + LIST_OFFSET + needed <= window.bottom();
        let top = if fits_below { bounds.h + LIST_OFFSET } else { -(needed + LIST_OFFSET) };
        Placement { open: true, top }
    }
}

enum ParamDropdownEvent {
    Select(f32),
}

impl View for ParamDropdown {
    fn element(&self) -> Option<&'static str> {
        Some("param-dropdown")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                // Presses on the open list bubble up through here on their way
                // to nowhere, and toggling on those would shut the list before
                // the item's own release could pick anything. The list hangs
                // outside the box, so the pointer's position tells them apart.
                let bounds = cx.bounds();
                let (x, y) = (cx.mouse().cursorx, cx.mouse().cursory);
                let on_the_box =
                    x >= bounds.x && x <= bounds.x + bounds.w && y >= bounds.y && y <= bounds.y + bounds.h;
                if !on_the_box {
                    return;
                }
                self.placement =
                    if self.placement.open { Placement::default() } else { self.placement_for(cx) };
                cx.emit(PlacementEvent::Set(self.placement));
                meta.consume();
            }
        });

        event.map(|dropdown_event, meta| {
            let ParamDropdownEvent::Select(normalized) = dropdown_event;
            // A picked value is one gesture, so it is bracketed like any other edit.
            self.param_base.begin_set_parameter(cx);
            self.param_base.set_normalized_value(cx, *normalized);
            self.param_base.end_set_parameter(cx);
            self.placement = Placement::default();
            meta.consume();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_is_as_tall_as_its_rows_plus_its_inset() {
        // The open list's height is worked out before it exists, so this is the
        // number that decides whether it opens downward or upward.
        assert_eq!(list_height(1), ITEM_HEIGHT + LIST_PADDING * 2.0);
        assert_eq!(list_height(4), 4.0 * ITEM_HEIGHT + 3.0 * LIST_GAP + LIST_PADDING * 2.0);
        // Never negative, whatever a parameter reports.
        assert!(list_height(0) >= 0.0);
    }

    #[test]
    fn list_height_grows_with_every_entry() {
        for count in 1..12 {
            assert!(
                list_height(count + 1) > list_height(count),
                "{count} entries were not shorter than {}",
                count + 1
            );
        }
    }
}
