//! Draggable AHDSR curve editor.
//!
//! Reproduces the design's ENV panel: a dark well with a cyan envelope, dashed
//! stage dividers, and circular handles that write straight back to the
//! parameters. The curve itself comes from `EnvelopeSettings::stage_level`, the
//! same function the audio path uses, so what is drawn is what is heard.

use ds_dsp::envelope::Stage;
use ds_dsp::EnvelopeSettings;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::param_base::ParamWidgetBase;
use nih_plug_vizia::widgets::util::ModifiersExt;

use crate::params::{DefaultSynthParams, EnvParams};

/// Horizontal share of the well given to the sustain plateau. The sustain stage
/// has no duration of its own, so it gets a fixed slice rather than a scaled one.
const SUSTAIN_WIDTH_FRACTION: f32 = 0.18;
/// Curve resolution per timed stage.
const SEGMENT_POINTS: usize = 48;
/// Hit radius around a handle, in pixels.
const HANDLE_GRAB_RADIUS: f32 = 11.0;
const HANDLE_RADIUS: f32 = 5.0;
/// Fine-drag divisor while Shift is held.
const GRANULAR_MULTIPLIER: f32 = 0.15;

/// Which handle a gesture is moving.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Grip {
    Attack,
    Hold,
    /// Horizontal moves decay, vertical moves sustain.
    DecaySustain,
    Release,
}

#[derive(Clone, Copy)]
struct DragState {
    grip: Grip,
    anchor_x: f32,
    anchor_time: f32,
    granular: bool,
}

pub struct EnvelopeDisplay {
    attack: ParamWidgetBase,
    hold: ParamWidgetBase,
    decay: ParamWidgetBase,
    sustain: ParamWidgetBase,
    release: ParamWidgetBase,
    drag: Option<DragState>,
}

/// Where each stage boundary lands inside the well.
struct Layout {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    /// x of the end of attack, hold, decay, sustain plateau and release.
    attack_x: f32,
    hold_x: f32,
    decay_x: f32,
    sustain_x: f32,
    release_x: f32,
    sustain: f32,
}

impl Layout {
    fn level_to_y(&self, level: f32) -> f32 {
        self.top + self.height * (1.0 - level.clamp(0.0, 1.0))
    }
}

impl EnvelopeDisplay {
    pub fn new<L>(cx: &mut Context, params: L, select: fn(&DefaultSynthParams) -> &EnvParams) -> Handle<'_, Self>
    where
        L: Lens<Target = std::sync::Arc<DefaultSynthParams>> + Clone,
    {
        Self {
            attack: ParamWidgetBase::new(cx, params.clone(), move |p| &select(p).attack),
            hold: ParamWidgetBase::new(cx, params.clone(), move |p| &select(p).hold),
            decay: ParamWidgetBase::new(cx, params.clone(), move |p| &select(p).decay),
            sustain: ParamWidgetBase::new(cx, params.clone(), move |p| &select(p).sustain),
            release: ParamWidgetBase::new(cx, params, move |p| &select(p).release),
            drag: None,
        }
        .build(cx, |_| {})
    }

    fn settings(&self) -> EnvelopeSettings {
        EnvelopeSettings {
            attack: self.attack.unmodulated_plain_value(),
            hold: self.hold.unmodulated_plain_value(),
            decay: self.decay.unmodulated_plain_value(),
            sustain: self.sustain.unmodulated_plain_value(),
            release: self.release.unmodulated_plain_value(),
        }
    }

    /// Splits the well between the timed stages in proportion to their length.
    ///
    /// A long release genuinely should dwarf a 5 ms attack, which is what the
    /// reference design shows, so the mapping stays proportional rather than
    /// compressed. Each stage keeps a minimum sliver so a zero-length one is
    /// still visible and grabbable.
    fn layout(&self, bounds: BoundingBox) -> Layout {
        let pad = 10.0_f32.min(bounds.h * 0.12);
        let left = bounds.x + pad;
        let top = bounds.y + pad;
        let width = (bounds.w - pad * 2.0).max(1.0);
        let height = (bounds.h - pad * 2.0).max(1.0);

        let settings = self.settings();
        let timed_width = width * (1.0 - SUSTAIN_WIDTH_FRACTION);
        let total = settings.attack + settings.hold + settings.decay + settings.release;
        let min_share = 6.0_f32.min(timed_width / 8.0);

        let share = |time: f32| {
            if total <= f32::EPSILON {
                timed_width / 4.0
            } else {
                min_share + (timed_width - min_share * 4.0).max(0.0) * (time / total)
            }
        };
        let attack_w = share(settings.attack);
        // Hold alone collapses to nothing when it is zero rather than keeping a
        // minimum sliver. The other stages keep theirs so their handles stay
        // grabbable, but hold's handle is hidden at zero, so a sliver would only
        // leave a plateau in the curve with nothing to explain it.
        let hold_w = if settings.hold <= f32::EPSILON { 0.0 } else { share(settings.hold) };
        let decay_w = share(settings.decay);
        let release_w = share(settings.release);

        let attack_x = left + attack_w;
        let hold_x = attack_x + hold_w;
        let decay_x = hold_x + decay_w;
        let sustain_x = decay_x + width * SUSTAIN_WIDTH_FRACTION;
        Layout {
            left,
            top,
            width,
            height,
            attack_x,
            hold_x,
            decay_x,
            sustain_x,
            release_x: sustain_x + release_w,
            sustain: settings.sustain,
        }
    }

    /// The handles that are currently on screen.
    ///
    /// Hold drops out at zero: it would sit exactly on top of the attack handle,
    /// so all it could do is intercept drags meant for the attack. The HOLD knob
    /// under the well is still there to bring it back.
    fn handle_positions(&self, layout: &Layout) -> Vec<(Grip, f32, f32)> {
        let mut handles = vec![(Grip::Attack, layout.attack_x, layout.level_to_y(1.0))];
        if self.hold.unmodulated_plain_value() > f32::EPSILON {
            handles.push((Grip::Hold, layout.hold_x, layout.level_to_y(1.0)));
        }
        handles.push((Grip::DecaySustain, layout.decay_x, layout.level_to_y(layout.sustain)));
        handles.push((Grip::Release, layout.release_x, layout.level_to_y(0.0)));
        handles
    }

    fn param_for(&self, grip: Grip) -> &ParamWidgetBase {
        match grip {
            Grip::Attack => &self.attack,
            Grip::Hold => &self.hold,
            Grip::DecaySustain => &self.decay,
            Grip::Release => &self.release,
        }
    }

    fn begin_drag(&mut self, cx: &mut EventContext, grip: Grip, x: f32) {
        cx.capture();
        cx.set_active(true);
        self.param_for(grip).begin_set_parameter(cx);
        if grip == Grip::DecaySustain {
            self.sustain.begin_set_parameter(cx);
        }
        self.drag = Some(DragState {
            grip,
            anchor_x: x,
            anchor_time: self.param_for(grip).unmodulated_normalized_value(),
            granular: cx.modifiers().shift(),
        });
    }

    fn end_drag(&mut self, cx: &mut EventContext) {
        if let Some(drag) = self.drag.take() {
            self.param_for(drag.grip).end_set_parameter(cx);
            if drag.grip == Grip::DecaySustain {
                self.sustain.end_set_parameter(cx);
            }
            cx.release();
            cx.set_active(false);
        }
    }
}

impl View for EnvelopeDisplay {
    fn element(&self) -> Option<&'static str> {
        Some("envelope-display")
    }

    fn event(&mut self, cx: &mut EventContext, event: &mut Event) {
        event.map(|window_event, meta| match window_event {
            WindowEvent::MouseDown(MouseButton::Left) => {
                let (x, y) = (cx.mouse().cursorx, cx.mouse().cursory);
                let layout = self.layout(cx.bounds());
                let hit = self
                    .handle_positions(&layout)
                    .into_iter()
                    .map(|(grip, hx, hy)| (grip, (hx - x).hypot(hy - y)))
                    .filter(|(_, distance)| *distance <= HANDLE_GRAB_RADIUS)
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                if let Some((grip, _)) = hit {
                    self.begin_drag(cx, grip, x);
                    meta.consume();
                }
            }
            WindowEvent::MouseDoubleClick(MouseButton::Left) => {
                // Reset whichever handle is under the pointer to its default.
                let (x, y) = (cx.mouse().cursorx, cx.mouse().cursory);
                let layout = self.layout(cx.bounds());
                self.end_drag(cx);
                for (grip, hx, hy) in self.handle_positions(&layout) {
                    if (hx - x).hypot(hy - y) <= HANDLE_GRAB_RADIUS {
                        let param = self.param_for(grip);
                        param.begin_set_parameter(cx);
                        param.set_normalized_value(cx, param.default_normalized_value());
                        param.end_set_parameter(cx);
                        meta.consume();
                        break;
                    }
                }
            }
            WindowEvent::MouseUp(MouseButton::Left) => {
                self.end_drag(cx);
                meta.consume();
            }
            WindowEvent::MouseMove(x, y) => {
                let Some(drag) = self.drag else { return };
                let bounds = cx.bounds();
                let layout = self.layout(bounds);
                let granular = cx.modifiers().shift();

                // Time parameters are heavily skewed, so a pixel does not map onto a
                // fixed number of seconds. Dragging therefore moves the parameter in
                // normalised space: crossing the well spans the full range.
                let mut drag = drag;
                if drag.granular != granular {
                    drag.granular = granular;
                    drag.anchor_x = *x;
                    drag.anchor_time = self.param_for(drag.grip).unmodulated_normalized_value();
                }
                let multiplier = if granular { GRANULAR_MULTIPLIER } else { 1.0 };
                let delta = (*x - drag.anchor_x) / layout.width.max(1.0) * multiplier;
                let param = self.param_for(drag.grip);
                param.set_normalized_value(cx, (drag.anchor_time + delta).clamp(0.0, 1.0));
                self.drag = Some(drag);

                // Sustain is a plain 0..1 level, so it can track the cursor exactly.
                if drag.grip == Grip::DecaySustain {
                    let level = 1.0 - (*y - layout.top) / layout.height.max(1.0);
                    self.sustain.set_normalized_value(cx, level.clamp(0.0, 1.0));
                }
                meta.consume();
            }
            // No MouseOut handler: the pointer is expected to leave the well while
            // dragging a handle, and `cx.capture()` keeps delivering events anyway.
            _ => {}
        });
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w <= 4.0 || bounds.h <= 4.0 {
            return;
        }

        let mut path = cx.build_path();
        // The default View::draw runs shadows before the background; overriding
        // draw means doing that here too, or the CSS box-shadow never appears.
        cx.draw_shadows(canvas, &mut path);
        cx.draw_background(canvas, &mut path);

        let opacity = cx.opacity();
        let settings = self.settings();
        let layout = self.layout(bounds);

        let mut curve: vg::Color = cx.font_color().into();
        curve.set_alphaf(curve.a * opacity);
        let mut accent: vg::Color = cx.selection_color().into();
        accent.set_alphaf(accent.a * opacity);
        let mut divider: vg::Color = cx.border_color().into();
        divider.set_alphaf(divider.a * opacity * 0.7);

        // Dashed stage dividers, as in the design.
        for x in [layout.attack_x, layout.hold_x, layout.decay_x, layout.sustain_x] {
            let mut path = vg::Path::new();
            let mut y = layout.top;
            while y < layout.top + layout.height {
                path.move_to(x, y);
                path.line_to(x, (y + 4.0).min(layout.top + layout.height));
                y += 8.0;
            }
            let mut paint = vg::Paint::color(divider);
            paint.set_line_width(1.0);
            canvas.stroke_path(&path, &paint);
        }

        // The envelope itself, sampled from the same curve the audio uses.
        let mut path = vg::Path::new();
        path.move_to(layout.left, layout.level_to_y(0.0));
        let segment = |from_x: f32, to_x: f32, stage: Stage, path: &mut vg::Path| {
            for step in 0..=SEGMENT_POINTS {
                let progress = step as f32 / SEGMENT_POINTS as f32;
                let x = from_x + (to_x - from_x) * progress;
                path.line_to(x, layout.level_to_y(settings.stage_level(stage, progress)));
            }
        };
        segment(layout.left, layout.attack_x, Stage::Attack, &mut path);
        segment(layout.attack_x, layout.hold_x, Stage::Hold, &mut path);
        segment(layout.hold_x, layout.decay_x, Stage::Decay, &mut path);
        segment(layout.decay_x, layout.sustain_x, Stage::Sustain, &mut path);
        segment(layout.sustain_x, layout.release_x, Stage::Release, &mut path);

        let mut paint = vg::Paint::color(accent);
        paint.set_line_width(2.0);
        paint.set_line_cap(vg::LineCap::Round);
        paint.set_line_join(vg::LineJoin::Round);
        canvas.stroke_path(&path, &paint);

        // Handles: filled when being dragged, hollow otherwise.
        for (grip, x, y) in self.handle_positions(&layout) {
            let active = self.drag.map(|drag| drag.grip) == Some(grip);
            let mut path = vg::Path::new();
            path.circle(x, y, HANDLE_RADIUS);
            if active {
                canvas.fill_path(&path, &vg::Paint::color(accent));
            } else {
                canvas.fill_path(&path, &vg::Paint::color(curve));
                let mut paint = vg::Paint::color(accent);
                paint.set_line_width(2.0);
                canvas.stroke_path(&path, &paint);
            }
        }
    }
}
