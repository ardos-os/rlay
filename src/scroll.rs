use crate::geometry::Vector;
use crate::input::{InputState, PointerGesture, PointerId, TouchPhase};
use crate::result::{LayoutResult, ScrollData};
use std::collections::HashMap;

type QueryScrollOffset = dyn Fn(&str) -> Vector;

const OVERSCROLL_RETURN_SPEED: f32 = 0.1;

const OVERSCROLL_DRAG_RATIO: f32 = 0.5;
const OVERSCROLL_SNAP_EPSILON: f32 = 0.05;
const SCROLL_FRICTION: f32 = 0.90;
const SCROLL_STOP_SPEED: f32 = 0.1;
const TOUCH_SCROLL_SLOP: f32 = 6.0;

struct ActiveScroll {
    scroll_id: String,
    initial_offset: Vector,
}

pub(crate) struct ScrollState {
    offsets: HashMap<String, Vector>,
    momentum: HashMap<String, Vector>,
    active_pointers: HashMap<PointerId, ActiveScroll>,
    query_offset: Option<Box<QueryScrollOffset>>,
    drag_scrolling: bool,
    wheel_scrolling: bool,
    wheel_scroll_ended: bool,
}

impl Default for ScrollState {
    fn default() -> Self {
        Self {
            offsets: HashMap::new(),
            momentum: HashMap::new(),
            active_pointers: HashMap::new(),
            query_offset: None,
            drag_scrolling: true,
            wheel_scrolling: false,
            wheel_scroll_ended: false,
        }
    }
}

impl ScrollState {
    pub(crate) fn set_drag_scrolling(&mut self, enabled: bool) {
        self.drag_scrolling = enabled;
    }

    pub(crate) fn set_offset(&mut self, id: impl Into<String>, offset: Vector) {
        self.offsets.insert(id.into(), offset);
    }

    pub(crate) fn set_momentum(&mut self, id: impl Into<String>, velocity: Vector) {
        self.set_momentum_inner(id.into(), velocity);
    }

    pub(crate) fn set_query_offset(&mut self, query: impl Fn(&str) -> Vector + 'static) {
        self.query_offset = Some(Box::new(query));
    }

    pub(crate) fn offset(&self, id: &str) -> Vector {
        if let Some(query) = &self.query_offset {
            return query(id);
        }
        self.offsets.get(id).copied().unwrap_or(Vector::ZERO)
    }

    pub(crate) fn apply_input_scroll(&mut self, input: &mut InputState, result: &LayoutResult) {
        self.update_offsets_from_input(input, result, true);
    }

    pub(crate) fn finish_layout_frame(&mut self, input: &mut InputState, result: &LayoutResult) {
        self.update_offsets_from_input(input, result, false);
        self.update_momentum(input, result);
        self.wheel_scroll_ended = false;
    }

    #[allow(clippy::too_many_lines)]
    fn update_offsets_from_input(
        &mut self,
        input: &mut InputState,
        result: &LayoutResult,
        consume: bool,
    ) {
        let mut wheel_inputs: HashMap<String, (Vector, Option<TouchPhase>)> = HashMap::new();
        let mut pointers: Vec<_> = input.pointers().collect();
        pointers.sort_by_key(|pointer| pointer.sequence);

        let mut consumed = Vec::new();

        self.active_pointers.retain(|pointer_id, _| {
            pointers
                .iter()
                .any(|pointer| pointer.id == *pointer_id && pointer.phase.is_down())
        });

        for pointer in pointers {
            let hovered_scroll_id = result
                .scroll_containers
                .iter()
                .find(|(_, scroll)| scroll.bounds.contains(pointer.position))
                .map(|(id, _)| id.clone());

            if let Some(id) = hovered_scroll_id.clone() {
                if pointer.scroll_delta != Vector::ZERO || pointer.scroll_phase.is_some() {
                    consumed.push(pointer.id);
                }
                let entry = wheel_inputs.entry(id).or_insert((Vector::ZERO, None));

                entry.0.x += pointer.scroll_delta.x;
                entry.0.y += pointer.scroll_delta.y;
                entry.1 = pointer.scroll_phase.or(entry.1);
            }

            if !self.drag_scrolling
                || !matches!(pointer.id, PointerId::Touch(_))
                || !pointer.phase.is_down()
            {
                continue;
            }

            let total_drag = Vector::new(
                pointer.position.x - pointer.start_position.x,
                pointer.position.y - pointer.start_position.y,
            );

            let scroll_id = self
                .active_pointers
                .get(&pointer.id)
                .map(|active| active.scroll_id.clone())
                .or(hovered_scroll_id);

            let Some(scroll_id) = scroll_id else {
                continue;
            };
            if self
                .active_pointers
                .iter()
                .any(|(id, active)| *id != pointer.id && active.scroll_id == scroll_id)
            {
                continue;
            }
            let Some(scroll) = result.scroll_containers.get(&scroll_id) else {
                continue;
            };
            let is_scroll =
                pointer.gesture == PointerGesture::Scroll || drag_passes_slop(scroll, total_drag);

            if !is_scroll {
                continue;
            }
            let current_scroll_offset = scroll.offset;
            let active = self.active_pointers.entry(pointer.id).or_insert_with(|| {
                self.momentum.remove(&scroll_id);
                ActiveScroll {
                    scroll_id: scroll_id.clone(),
                    initial_offset: current_scroll_offset,
                }
            });
            let active_scroll_id = active.scroll_id.clone();

            input.mark_scroll_gesture(pointer.id);

            let limit = scroll_limit(scroll);
            let next = Vector::new(
                if scroll.scroll_x {
                    drag_axis(active.initial_offset.x, -total_drag.x, limit.x)
                } else {
                    current_scroll_offset.x
                },
                if scroll.scroll_y {
                    drag_axis(active.initial_offset.y, -total_drag.y, limit.y)
                } else {
                    current_scroll_offset.y
                },
            );

            self.offsets.insert(active.scroll_id.clone(), next);

            let frame_delta = Vector::new(
                next.x - current_scroll_offset.x,
                next.y - current_scroll_offset.y,
            );

            if frame_delta != Vector::ZERO {
                self.set_momentum_inner(active_scroll_id, frame_delta);
            }

            consumed.push(pointer.id);
        }

        for (id, (wheel_delta, wheel_phase)) in wheel_inputs {
            let Some(scroll) = result.scroll_containers.get(&id) else {
                continue;
            };

            if wheel_phase.is_some() {
                self.apply_phased_wheel_scroll(id, wheel_delta, wheel_phase, scroll);
            } else {
                self.apply_wheel_scroll(id, wheel_delta, scroll);
            }
        }

        if consume {
            consumed.sort_by_key(|id| match id {
                PointerId::Mouse => 0,
                PointerId::Touch(id) => *id + 1,
            });
            consumed.dedup();

            for id in consumed {
                input.consume_pointer_delta(id);
            }
        }
    }

    fn apply_phased_wheel_scroll(
        &mut self,
        id: String,
        wheel_delta: Vector,
        wheel_phase: Option<TouchPhase>,
        scroll: &ScrollData,
    ) {
        let current = scroll.offset;
        let limit = scroll_limit(scroll);
        match wheel_phase {
            Some(TouchPhase::Started) => {
                self.wheel_scrolling = true;
                self.wheel_scroll_ended = false;
                self.momentum.remove(&id);
            }
            Some(TouchPhase::Moved) => {
                self.wheel_scrolling = true;
            }
            Some(TouchPhase::Ended | TouchPhase::Cancelled) => {
                self.wheel_scrolling = false;
                self.wheel_scroll_ended = true;
            }
            None => {}
        }

        if matches!(wheel_phase, Some(TouchPhase::Started | TouchPhase::Moved)) {
            let next = Vector::new(
                if scroll.scroll_x {
                    drag_axis(current.x, wheel_delta.x, limit.x)
                } else {
                    current.x
                },
                if scroll.scroll_y {
                    drag_axis(current.y, wheel_delta.y, limit.y)
                } else {
                    current.y
                },
            );
            self.offsets.insert(id.clone(), next);
            self.set_momentum_inner(id, Vector::new(next.x - current.x, next.y - current.y));
        }
    }

    fn apply_wheel_scroll(&mut self, id: String, wheel_delta: Vector, scroll: &ScrollData) {
        if wheel_delta == Vector::ZERO {
            return;
        }

        self.set_momentum_inner(id.clone(), wheel_delta);

        let limit = scroll_limit(scroll);
        let current = self.offset(&id);

        self.offsets.insert(
            id,
            Vector::new(
                if scroll.scroll_x {
                    (current.x + wheel_delta.x).clamp(0.0, limit.x)
                } else {
                    current.x
                },
                if scroll.scroll_y {
                    (current.y + wheel_delta.y).clamp(0.0, limit.y)
                } else {
                    current.y
                },
            ),
        );
    }

    fn update_momentum(&mut self, input: &InputState, result: &LayoutResult) {
        if self.wheel_scrolling
            || self.wheel_scroll_ended
            || input.pointers().any(|pointer| pointer.phase.is_down())
        {
            return;
        }

        let ids: Vec<_> = self.momentum.keys().cloned().collect();
        for id in ids {
            let Some(scroll) = result.scroll_containers.get(&id) else {
                self.momentum.remove(&id);
                continue;
            };
            let velocity = self.momentum.get(&id).copied().unwrap_or(Vector::ZERO);
            let limit = scroll_limit(scroll);
            let current = self.offset(&id);

            let (x, vx, done_x) = if scroll.scroll_x {
                free_axis(current.x, velocity.x, limit.x)
            } else {
                (current.x, 0.0, true)
            };
            let (y, vy, done_y) = if scroll.scroll_y {
                free_axis(current.y, velocity.y, limit.y)
            } else {
                (current.y, 0.0, true)
            };

            self.offsets.insert(id.clone(), Vector::new(x, y));

            if done_x && done_y {
                self.momentum.remove(&id);
            } else {
                self.momentum.insert(id, Vector::new(vx, vy));
            }
        }
    }

    fn set_momentum_inner(&mut self, id: String, velocity: Vector) {
        self.momentum.insert(id, velocity);
    }
}

fn scroll_limit(scroll: &ScrollData) -> Vector {
    Vector::new(
        (scroll.content_size.width - scroll.bounds.width).max(0.0),
        (scroll.content_size.height - scroll.bounds.height).max(0.0),
    )
}

fn drag_axis(initial: f32, delta: f32, max: f32) -> f32 {
    let initial = if initial < 0.0 {
        initial / OVERSCROLL_DRAG_RATIO
    } else if initial > max {
        max + (initial - max) / OVERSCROLL_DRAG_RATIO
    } else {
        initial
    };
    let next = initial + delta;

    if next < 0.0 {
        next * OVERSCROLL_DRAG_RATIO
    } else if next > max {
        max + (next - max) * OVERSCROLL_DRAG_RATIO
    } else {
        next
    }
}

fn drag_passes_slop(scroll: &ScrollData, delta: Vector) -> bool {
    match (scroll.scroll_x, scroll.scroll_y) {
        (true, true) => delta.x.abs().max(delta.y.abs()) >= TOUCH_SCROLL_SLOP,
        (true, false) => delta.x.abs() >= TOUCH_SCROLL_SLOP,
        (false, true) => delta.y.abs() >= TOUCH_SCROLL_SLOP,
        (false, false) => false,
    }
}

fn return_delta(value: f32, max: f32) -> f32 {
    if value < 0.0 {
        -value
    } else if value > max {
        max - value
    } else {
        0.0
    }
}

fn free_axis(value: f32, velocity: f32, max: f32) -> (f32, f32, bool) {
    let distance = return_delta(value, max);
    if distance == 0.0 {
        let velocity = velocity * SCROLL_FRICTION;
        if velocity.abs() < SCROLL_STOP_SPEED {
            (value.clamp(0.0, max), 0.0, true)
        } else {
            (value + velocity, velocity, false)
        }
    } else {
        let velocity = move_towards(velocity, distance);
        let next = value + velocity;
        let target = value.clamp(0.0, max);
        if distance.abs() <= OVERSCROLL_SNAP_EPSILON
            || (value < 0.0 && next >= 0.0)
            || (value > max && next <= max)
        {
            (target, 0.0, true)
        } else {
            (next, velocity, false)
        }
    }
}
fn sigmoid01(t: f32) -> f32 {
    1.0 / (1.0 + (-12.0 * (t - 0.5)).exp())
}
fn move_towards(velocity: f32, distance: f32) -> f32 {
    let target_velocity = distance * OVERSCROLL_RETURN_SPEED;

    let delta = target_velocity - velocity;
    let mut acceleration = (distance.abs() * OVERSCROLL_RETURN_SPEED).sqrt().max(0.01);

    if velocity.signum() != distance.signum() {
        acceleration *= sigmoid01(distance.abs()).sqrt() * 5.;
    }
    velocity + delta.clamp(-acceleration, acceleration)
}
