/// A renderer-facing command emitted by layout.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandKind {
    /// Draw a filled rectangle.
    Rectangle {
        /// Fill color.
        color: Color,
        /// Corner radii.
        radius: Radius,
    },
    /// Draw a border.
    Border(Border),
    /// Draw a text run.
    Text {
        /// Text contents.
        text: String,
        /// Text style. Command bounds describe the line box; renderers must
        /// derive the glyph baseline from their font metrics.
        style: TextStyle,
    },
    /// Draw an application-owned image.
    Image(u64),
    /// Draw an application-owned custom element.
    Custom(u64, Radius),
    /// Start clipping subsequent commands.
    ClipStart {
        /// Clip horizontally.
        x: bool,
        /// Clip vertically.
        y: bool,
    },
    /// End the current clip scope.
    ClipEnd,
    /// Start an overlay scope.
    OverlayStart(Color),
    /// End the current overlay scope.
    OverlayEnd,
    /// Draw or inspect debug information.
    DebugOverlay {
        /// Number of elements in the result.
        elements: usize,
        /// Number of commands in the result.
        commands: usize,
    },
}

/// A single ordered render command.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderCommand {
    /// Id of the source node, when present.
    pub id: Option<String>,
    /// Command bounds in viewport coordinates.
    pub bounds: Rect,
    /// Command payload.
    pub kind: CommandKind,
}

/// Query data for a laid-out element.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementData {
    /// Element bounds in viewport coordinates.
    pub bounds: Rect,
    /// Stable id metadata, when present.
    pub element_id: Option<ElementId>,
}

/// Query data for a scroll container.
#[derive(Debug, Clone, PartialEq)]
pub struct ScrollData {
    /// Visible scroll container bounds.
    pub bounds: Rect,
    /// Total scrollable content size.
    pub content_size: Size,
    /// Current scroll offset.
    pub offset: Vector,
    /// Whether horizontal scrolling is enabled.
    pub scroll_x: bool,
    /// Whether vertical scrolling is enabled.
    pub scroll_y: bool,
}

/// Complete output of one layout pass.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LayoutResult {
    /// Ordered commands to render.
    pub commands: Vec<RenderCommand>,
    /// Element data keyed by string id.
    pub elements: HashMap<String, ElementData>,
    /// Scroll container data keyed by string id.
    pub scroll_containers: HashMap<String, ScrollData>,
    /// Pointer hit information for this frame.
    pub pointers: Vec<PointerHit>,
    /// Recoverable layout errors collected during the pass.
    pub errors: Vec<LayoutError>,
    pub(crate) hit_order: Vec<HitEntry>,
}

impl LayoutResult {
    /// Returns element data by id.
    #[must_use]
    pub fn element(&self, id: &str) -> Option<&ElementData> {
        self.elements.get(id)
    }

    /// Returns pointer hits using this layout and the current input state.
    #[must_use]
    pub fn pointer_hits(&self, input_state: &InputState) -> Vec<PointerHit> {
        input_state
            .pointers()
            .map(|pointer| PointerHit {
                pointer_id: pointer.id,
                position: pointer.position,
                phase: pointer.phase,
                element_id: input_state
                    .pointer_capture(pointer.id)
                    .map(str::to_owned)
                    .or_else(|| self.hit_test(pointer.position).map(str::to_owned)),
                mouse_button: pointer.mouse_button,
                gesture: pointer.gesture,
            })
            .collect()
    }

    fn hit_test(&self, point: Point) -> Option<&str> {
        self.hit_order
            .iter()
            .rev()
            .find(|entry| entry.bounds.contains(point))
            .map(|entry| entry.id.as_str())
    }

    /// Returns true if any pointer is currently over the element id.
    #[must_use]
    pub fn pointer_over(&self, id: &str) -> bool {
        self.pointers
            .iter()
            .any(|pointer| pointer.element_id.as_deref() == Some(id))
    }

    /// Returns true if any pointer is currently pressing the element id.
    #[must_use]
    pub fn element_is_pressed(&self, id: &str) -> bool {
        self.pointers
            .iter()
            .any(|pointer| pointer.element_id.as_deref() == Some(id) && pointer.phase.is_down())
    }

    /// Returns true if any pointer is currently over the element id.
    pub fn get_pointers_pressing(&self, id: &str) -> impl Iterator<Item = &PointerHit> {
        self.pointers.iter().filter(move |pointer| {
            pointer.element_id.as_deref() == Some(id) && pointer.phase.is_down()
        })
    }
    /// Returns unique element ids currently under pointers.
    #[must_use]
    pub fn pointer_over_ids(&self) -> Vec<&str> {
        let mut ids = Vec::new();
        for pointer in &self.pointers {
            let Some(id) = pointer.element_id.as_deref() else {
                continue;
            };
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        ids
    }

    /// Returns scroll data by id.
    #[must_use]
    pub fn scroll_container(&self, id: &str) -> Option<&ScrollData> {
        self.scroll_containers.get(id)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HitEntry {
    pub(crate) id: String,
    pub(crate) bounds: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct ClipRegion {
    left: Option<f32>,
    right: Option<f32>,
    top: Option<f32>,
    bottom: Option<f32>,
}

impl ClipRegion {
    pub(crate) fn add(self, bounds: Rect, clip_x: bool, clip_y: bool) -> Self {
        Self {
            left: if clip_x {
                Some(self.left.map_or(bounds.x, |left| left.max(bounds.x)))
            } else {
                self.left
            },
            right: if clip_x {
                let right = bounds.x + bounds.width;
                Some(self.right.map_or(right, |old| old.min(right)))
            } else {
                self.right
            },
            top: if clip_y {
                Some(self.top.map_or(bounds.y, |top| top.max(bounds.y)))
            } else {
                self.top
            },
            bottom: if clip_y {
                let bottom = bounds.y + bounds.height;
                Some(self.bottom.map_or(bottom, |old| old.min(bottom)))
            } else {
                self.bottom
            },
        }
    }

    pub(crate) fn apply(self, bounds: Rect) -> Option<Rect> {
        let left = self.left.unwrap_or(bounds.x).max(bounds.x);
        let right = self
            .right
            .unwrap_or(bounds.x + bounds.width)
            .min(bounds.x + bounds.width);
        let top = self.top.unwrap_or(bounds.y).max(bounds.y);
        let bottom = self
            .bottom
            .unwrap_or(bounds.y + bounds.height)
            .min(bounds.y + bounds.height);

        (right > left && bottom > top).then(|| Rect::new(left, top, right - left, bottom - top))
    }
}
use crate::engine::LayoutError;
use crate::geometry::{Color, Point, Radius, Rect, Size, Vector};
use crate::id::ElementId;
use crate::input::{InputState, PointerHit};
use crate::style::{Border, TextStyle};
use std::collections::HashMap;
