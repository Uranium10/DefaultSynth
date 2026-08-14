//! Dragging a modulation source onto a knob.
//!
//! Two halves of one gesture, drawn exactly as the design has them:
//!
//! - [`ModHandle`] is the ✛ on each ENV and LFO tab, and above the NOTE and VELO
//!   boxes. Press it and you are carrying that source.
//! - [`ModDot`] is the small ring at a knob's upper left: grey when the knob can
//!   take a modulation, cyan when something is already routed to it. Release the
//!   drag over one and the routing is made.
//!
//! The dot deliberately does not capture the pointer. Capturing would send every
//! move to the handle, and then the drop would have to work out for itself which
//! knob is underneath — which means the handle would need to know where every
//! knob is. Letting the events go where they normally go means the knob under
//! the pointer is simply the one that receives the release.

use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::style::PointerEvents;
use nih_plug_vizia::vizia::vg;

use crate::params::{ModDestSlotParam, ModSourceSlotParam};
use crate::widgets::mod_router::ModRouteEvent;

/// Arm length of the ✛, in pixels.
const ARM: f32 = 5.0;
/// Half-width of an arrowhead at the end of an arm.
const HEAD: f32 = 2.6;
/// Radius of the ring beside a knob.
const RING_RADIUS: f32 = 4.2;
const RING_WIDTH: f32 = 1.8;

/// Whether a ring is currently accepting the pointer.
///
/// A ring sits on top of the knob it belongs to, so outside a drag it has to be
/// invisible to the mouse or the knob underneath would never turn. `PointerEvents`
/// is not something a lens can carry on its own, so this is the shape that can be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Data)]
pub struct DropTarget(pub bool);

impl From<DropTarget> for PointerEvents {
    fn from(active: DropTarget) -> Self {
        if active.0 {
            PointerEvents::Auto
        } else {
            PointerEvents::None
        }
    }
}

/// Raised while a source is being carried, so every drop target can light up.
pub enum ModDragEvent {
    Begin(ModSourceSlotParam),
    End,
}

/// The ✛ handle: press and drag it onto a knob's ring.
pub struct ModHandle {
    source: ModSourceSlotParam,
}

impl ModHandle {
    pub fn new(cx: &mut Context, source: ModSourceSlotParam) -> Handle<'_, Self> {
        Self { source }.build(cx, |_| {})
    }
}

impl View for ModHandle {
    fn element(&self) -> Option<&'static str> {
        Some("mod-handle")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| {
            if let WindowEvent::MouseDown(MouseButton::Left) = window_event {
                cx.emit(ModDragEvent::Begin(self.source));
                cx.needs_redraw();
                // Consumed so the tab underneath does not also switch pages: the
                // handle is for dragging, the rest of the tab is for selecting.
                meta.consume();
            }
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 2.0 || bounds.h <= 2.0 {
            return;
        }
        let mut colour: vg::Color = cx.font_color().into();
        colour.set_alphaf(colour.a * cx.opacity());

        let (cx_, cy) = (bounds.x + bounds.w / 2.0, bounds.y + bounds.h / 2.0);
        let arm = ARM.min(bounds.w / 2.0 - HEAD).min(bounds.h / 2.0 - HEAD).max(2.0);

        let mut path = vg::Path::new();
        path.move_to(cx_ - arm, cy);
        path.line_to(cx_ + arm, cy);
        path.move_to(cx_, cy - arm);
        path.line_to(cx_, cy + arm);
        let mut paint = vg::Paint::color(colour);
        paint.set_line_width(1.8);
        paint.set_line_cap(vg::LineCap::Round);
        canvas.stroke_path(&path, &paint);

        // Four arrowheads, which is what makes it read as "move me" rather than
        // as a plus sign.
        let mut heads = vg::Path::new();
        for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
            let (tip_x, tip_y) = (cx_ + dx * (arm + HEAD * 0.7), cy + dy * (arm + HEAD * 0.7));
            // Perpendicular to the arm, so each head sits square on its own arm.
            let (px, py) = (-dy, dx);
            heads.move_to(tip_x, tip_y);
            heads.line_to(cx_ + dx * arm + px * HEAD, cy + dy * arm + py * HEAD);
            heads.line_to(cx_ + dx * arm - px * HEAD, cy + dy * arm - py * HEAD);
            heads.close();
        }
        canvas.fill_path(&heads, &vg::Paint::color(colour));
    }
}

/// The ring beside a knob: shows whether it is modulated, and takes the drop.
pub struct ModDot<D, R> {
    destination: ModDestSlotParam,
    /// The source currently being carried, if any.
    drag: D,
    /// One bit per routed destination, published by the router.
    routed: R,
}

impl<D, R> ModDot<D, R>
where
    D: Lens<Target = Option<ModSourceSlotParam>>,
    R: Lens<Target = u32>,
{
    pub fn new(cx: &mut Context, destination: ModDestSlotParam, drag: D, routed: R) -> Handle<'_, Self> {
        Self { destination, drag, routed }.build(cx, |_| {})
    }

    fn is_routed(&self, cx: &DrawContext) -> bool {
        self.routed.get(cx) & (1 << self.destination.to_index()) != 0
    }
}

impl<D, R> View for ModDot<D, R>
where
    D: Lens<Target = Option<ModSourceSlotParam>>,
    R: Lens<Target = u32>,
{
    fn element(&self) -> Option<&'static str> {
        Some("mod-dot")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            // Releasing a carried source here is the drop.
            WindowEvent::MouseUp(MouseButton::Left) => {
                if let Some(source) = self.drag.get(cx) {
                    cx.emit(ModRouteEvent::Assign { source, destination: self.destination });
                    meta.consume();
                }
            }
            // Right-click unroutes, which is the only way back out of a routing
            // until the MATRIX page exists.
            WindowEvent::MouseDown(MouseButton::Right) => {
                cx.emit(ModRouteEvent::Clear(self.destination));
                meta.consume();
            }
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 2.0 || bounds.h <= 2.0 {
            return;
        }
        let opacity = cx.opacity();
        // selection-color is the lit ring, border-color the unlit one.
        let mut colour: vg::Color = if self.is_routed(cx) || self.drag.get(cx).is_some() {
            cx.selection_color().into()
        } else {
            cx.border_color().into()
        };
        colour.set_alphaf(colour.a * opacity);

        let radius = RING_RADIUS.min(bounds.w / 2.0 - RING_WIDTH).max(1.5);
        let mut path = vg::Path::new();
        path.circle(bounds.x + bounds.w / 2.0, bounds.y + bounds.h / 2.0, radius);
        let mut paint = vg::Paint::color(colour);
        paint.set_line_width(RING_WIDTH);
        canvas.stroke_path(&path, &paint);
    }
}
