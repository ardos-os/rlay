use crate::geometry::{Color, Padding, Point, Rect, Size};
use crate::style::{Direction, TextStyle};

pub(crate) fn main_axis(size: Size, direction: Direction) -> f32 {
    match direction {
        Direction::Row => size.width,
        Direction::Column => size.height,
    }
}

/// Character-index based single-line text selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextSelection {
    /// Fixed side of the selection drag.
    pub anchor: usize,
    /// Moving side of the selection drag.
    pub focus: usize,
}

impl TextSelection {
    /// Creates a selection from anchor and focus character indices.
    #[must_use]
    pub const fn new(anchor: usize, focus: usize) -> Self {
        Self { anchor, focus }
    }

    /// Returns `(start, end)` when the selection is non-empty.
    #[must_use]
    pub fn normalized(self) -> Option<(usize, usize)> {
        match self.anchor.cmp(&self.focus) {
            std::cmp::Ordering::Less => Some((self.anchor, self.focus)),
            std::cmp::Ordering::Greater => Some((self.focus, self.anchor)),
            std::cmp::Ordering::Equal => None,
        }
    }
}

/// Paint and geometry values captured for transitions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionValues {
    /// Element bounds.
    pub bounds: Rect,
    /// Background color.
    pub background: Color,
    /// Overlay color.
    pub overlay: Color,
    /// Border color.
    pub border_color: Color,
    /// Border widths.
    pub border_width: Padding,
}

/// Quadratic ease-out interpolation.
#[must_use]
pub fn ease_out(from: f32, to: f32, elapsed: f32, duration: f32) -> f32 {
    if duration <= 0.0 {
        return to;
    }
    let t = (elapsed / duration).clamp(0.0, 1.0);
    let eased = 1.0 - (1.0 - t) * (1.0 - t);
    from + (to - from) * eased
}

pub(crate) fn midpoint(a: Point, b: Point) -> Point {
    Point::new(f32::midpoint(a.x, b.x), f32::midpoint(a.y, b.y))
}

pub(crate) fn point_distance(a: Point, b: Point) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

pub(crate) fn char_index_to_byte(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map_or(text.len(), |(byte, _)| byte)
}

pub(crate) fn resolved_line_height(style: &TextStyle) -> f32 {
    if style.line_height > 0.0 {
        style.line_height
    } else {
        style.font_size
    }
}
