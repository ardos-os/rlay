/// Identifier for a pointer source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointerId {
    /// The mouse pointer.
    Mouse,
    /// A touchscreen contact with an application-provided id.
    Touch(u64),
}

/// Mouse button associated with a pointer press.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    /// Primary mouse button.
    Left,
    /// Secondary mouse button.
    Right,
    /// Middle mouse button.
    Middle,
    /// Platform-specific extra button.
    Other(u16),
}

/// Pointer lifecycle phase for a layout frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerPhase {
    /// Pointer is present but not pressed.
    Hover,
    /// Pointer became pressed this frame.
    PressedThisFrame,
    /// Pointer is held down.
    Pressed,
    /// Pointer was released this frame.
    ReleasedThisFrame,
    /// Pointer is no longer active and will be removed after the frame.
    Released,
}

/// Gesture currently winning for a pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointerGesture {
    /// The pointer is still eligible for a tap/click.
    Tap,
    /// The pointer has moved enough to become a scroll gesture.
    Scroll,
}

/// Phase of a wheel or touchpad scroll gesture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TouchPhase {
    /// Scroll gesture started.
    Started,
    /// Scroll gesture moved.
    Moved,
    /// Scroll gesture ended.
    Ended,
    /// Scroll gesture was cancelled.
    Cancelled,
}

impl PointerPhase {
    /// Returns true while the pointer is pressed.
    #[must_use]
    pub fn is_down(self) -> bool {
        matches!(self, Self::PressedThisFrame | Self::Pressed)
    }
}

/// Current state for one pointer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pointer {
    /// Pointer identifier.
    pub id: PointerId,
    /// Current pointer position.
    pub position: Point,
    /// Previous-frame pointer position.
    pub previous_position: Point,
    /// Position where the current press started.
    pub start_position: Point,
    /// Scroll wheel or trackpad delta accumulated for this frame.
    pub scroll_delta: Vector,
    /// Phase of the wheel or touchpad scroll gesture for this frame.
    pub scroll_phase: Option<TouchPhase>,
    /// Current pointer phase.
    pub phase: PointerPhase,
    /// Mouse button for mouse presses. Touch pointers use `None`.
    pub mouse_button: Option<MouseButton>,
    /// Gesture currently winning for this pointer.
    pub gesture: PointerGesture,
    /// Monotonic order in which this pointer contact started.
    pub sequence: u64,
}

/// Hit-test result for one pointer.
#[derive(Debug, Clone, PartialEq)]
pub struct PointerHit {
    /// Pointer identifier.
    pub pointer_id: PointerId,
    /// Pointer position used for hit testing.
    pub position: Point,
    /// Pointer phase for this frame.
    pub phase: PointerPhase,
    /// Topmost element id under the pointer, if any.
    pub element_id: Option<String>,
    /// Mouse button for mouse presses. Touch pointers use `None`.
    pub mouse_button: Option<MouseButton>,
    /// Gesture currently winning for this pointer.
    pub gesture: PointerGesture,
}

/// Two-finger pinch gesture information.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PinchGesture {
    /// Current midpoint between the two touches.
    pub center: Point,
    /// Previous midpoint between the two touches.
    pub previous_center: Point,
    /// Current distance divided by previous distance.
    pub scale: f32,
}

/// Mutable input state consumed by [`crate::Engine`].
#[derive(Debug, Clone, Default)]
pub struct InputState {
    pointers: HashMap<PointerId, Pointer>,
    captures: HashMap<PointerId, String>,
    next_sequence: u64,
}

impl InputState {
    /// Updates mouse hover position.
    pub fn set_mouse_position(&mut self, position: Point) {
        let previous = self.pointers.get(&PointerId::Mouse).copied();
        self.set_pointer(
            PointerId::Mouse,
            position,
            previous.is_some_and(|pointer| pointer.phase.is_down()),
            previous.and_then(|pointer| pointer.mouse_button),
        );
    }

    /// Updates mouse button state and position.
    pub fn set_mouse_down(&mut self, position: Point, down: bool) {
        self.set_mouse_button(position, MouseButton::Left, down);
    }

    /// Updates mouse button state and position.
    pub fn set_mouse_button(&mut self, position: Point, button: MouseButton, down: bool) {
        self.set_pointer(PointerId::Mouse, position, down, Some(button));
    }

    /// Updates a touch point.
    pub fn set_touch(&mut self, id: u64, position: Point, down: bool) {
        self.set_pointer(PointerId::Touch(id), position, down, None);
    }

    /// Releases a touch point at `position`.
    pub fn remove_touch(&mut self, id: u64, position: Point) {
        self.set_touch(id, position, false);
    }

    /// Adds scroll delta to a pointer for the next layout pass.
    pub fn add_scroll_delta(&mut self, id: PointerId, delta: Vector) {
        self.add_scroll_delta_with_phase(id, delta, None);
    }

    /// Adds scroll delta and phase to a pointer for the next layout pass.
    pub fn add_scroll_delta_with_phase(
        &mut self,
        id: PointerId,
        delta: Vector,
        phase: Option<TouchPhase>,
    ) {
        if let Some(pointer) = self.pointers.get_mut(&id) {
            pointer.scroll_delta.x += delta.x;
            pointer.scroll_delta.y += delta.y;
            pointer.scroll_phase = phase;
        }
    }

    /// Captures a pointer to an element id until released or removed.
    pub fn capture_pointer(&mut self, id: PointerId, element_id: impl Into<String>) {
        self.captures.insert(id, element_id.into());
    }

    /// Releases pointer capture.
    pub fn release_pointer_capture(&mut self, id: PointerId) {
        self.captures.remove(&id);
    }

    /// Returns the element currently capturing `id`.
    pub fn pointer_capture(&self, id: PointerId) -> Option<&str> {
        self.captures.get(&id).map(String::as_str)
    }

    /// Marks a pointer as a scroll gesture winner.
    pub fn mark_scroll_gesture(&mut self, id: PointerId) {
        if let Some(pointer) = self.pointers.get_mut(&id) {
            pointer.gesture = PointerGesture::Scroll;
        }
    }

    /// Consumes movement used by pre-layout gesture handling.
    pub fn consume_pointer_delta(&mut self, id: PointerId) {
        if let Some(pointer) = self.pointers.get_mut(&id) {
            pointer.previous_position = pointer.position;
            pointer.scroll_delta = Vector::ZERO;
            pointer.scroll_phase = None;
        }
    }

    /// Returns pinch information when exactly two touches are pressed.
    #[must_use]
    pub fn pinch(&self) -> Option<PinchGesture> {
        let touches: Vec<_> = self
            .pointers
            .values()
            .filter(|pointer| matches!(pointer.id, PointerId::Touch(_)) && pointer.phase.is_down())
            .copied()
            .collect();
        let [a, b] = touches.as_slice() else {
            return None;
        };

        let distance = point_distance(a.position, b.position);
        let previous_distance = point_distance(a.previous_position, b.previous_position);
        Some(PinchGesture {
            center: midpoint(a.position, b.position),
            previous_center: midpoint(a.previous_position, b.previous_position),
            scale: if previous_distance > 0.0 {
                distance / previous_distance
            } else {
                1.0
            },
        })
    }

    fn set_pointer(
        &mut self,
        id: PointerId,
        position: Point,
        down: bool,
        mouse_button: Option<MouseButton>,
    ) {
        let previous = self.pointers.get(&id).copied();
        let sequence = previous.map_or_else(
            || {
                let sequence = self.next_sequence;
                self.next_sequence += 1;
                sequence
            },
            |pointer| pointer.sequence,
        );
        let phase = match (previous.is_some_and(|p| p.phase.is_down()), down) {
            (false, false) => PointerPhase::Hover,
            (false, true) => PointerPhase::PressedThisFrame,
            (true, true) => PointerPhase::Pressed,
            (true, false) => PointerPhase::ReleasedThisFrame,
        };
        self.pointers.insert(
            id,
            Pointer {
                id,
                position,
                previous_position: previous.map_or(position, |p| p.position),
                start_position: if down {
                    previous.map_or(position, |p| p.start_position)
                } else {
                    position
                },
                scroll_delta: previous.map_or(Vector::ZERO, |p| p.scroll_delta),
                scroll_phase: previous.and_then(|p| p.scroll_phase),
                phase,
                mouse_button: if down {
                    mouse_button.or_else(|| previous.and_then(|p| p.mouse_button))
                } else {
                    mouse_button
                },
                gesture: if down {
                    previous.map_or(PointerGesture::Tap, |p| p.gesture)
                } else {
                    previous.map_or(PointerGesture::Tap, |p| p.gesture)
                },
                sequence,
            },
        );
    }

    /// Returns the pointers of this [`InputState`].
    pub fn pointers(&self) -> impl Iterator<Item = Pointer> + '_ {
        self.pointers.values().copied()
    }

    pub(crate) fn end_frame(&mut self) {
        self.pointers.retain(|_, pointer| {
            pointer.scroll_delta = Vector::ZERO;
            pointer.scroll_phase = None;
            pointer.phase = match pointer.phase {
                PointerPhase::PressedThisFrame => PointerPhase::Pressed,
                PointerPhase::ReleasedThisFrame => PointerPhase::Released,
                phase => phase,
            };
            pointer.phase != PointerPhase::Released
        });
        self.captures
            .retain(|pointer_id, _| self.pointers.contains_key(pointer_id));
    }
}
use crate::geometry::{Point, Vector};
use crate::text::{midpoint, point_distance};
use std::collections::HashMap;
