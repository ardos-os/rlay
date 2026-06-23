/// Size rule for one layout axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AxisSize {
    /// Use measured content size clamped by `min` and `max`.
    Fit {
        /// Minimum resolved size.
        min: f32,
        /// Maximum resolved size.
        max: f32,
    },
    /// Fill remaining parent space clamped by `min` and `max`.
    Grow {
        /// Minimum resolved size.
        min: f32,
        /// Maximum resolved size.
        max: f32,
    },
    /// Use a fraction of the parent axis.
    Percent(f32),
    /// Use an exact logical-pixel value.
    Fixed(f32),
}

impl AxisSize {
    /// Fit to content with no upper bound.
    pub const FIT: Self = Self::Fit {
        min: 0.0,
        max: f32::MAX,
    };
    /// Grow to available space with no upper bound.
    pub const GROW: Self = Self::Grow {
        min: 0.0,
        max: f32::MAX,
    };

    /// Creates a fixed axis size.
    #[must_use]
    pub const fn fixed(value: f32) -> Self {
        Self::Fixed(value)
    }

    pub(crate) fn resolve(self, parent: f32, fit: f32) -> f32 {
        match self {
            Self::Fit { min, max } => fit.clamp(min, max),
            Self::Grow { min, max } => parent.clamp(min, max),
            Self::Percent(percent) => (parent * percent.clamp(0.0, 1.0)).max(0.0),
            Self::Fixed(value) => value.max(0.0),
        }
    }

    pub(crate) fn is_grow(self) -> bool {
        matches!(self, Self::Grow { .. })
    }
}

impl Default for AxisSize {
    fn default() -> Self {
        Self::FIT
    }
}

/// Width and height sizing rules.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Sizing {
    /// Width rule.
    pub width: AxisSize,
    /// Height rule.
    pub height: AxisSize,
}

impl Sizing {
    /// Creates fixed width and height sizing.
    #[must_use]
    pub const fn fixed(width: f32, height: f32) -> Self {
        Self {
            width: AxisSize::Fixed(width),
            height: AxisSize::Fixed(height),
        }
    }
}

/// Main axis direction for child layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    /// Children are placed left to right.
    #[default]
    Row,
    /// Children are placed top to bottom.
    Column,
}

/// Horizontal alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignX {
    /// Align to the left edge.
    #[default]
    Left,
    /// Align to the horizontal center.
    Center,
    /// Align to the right edge.
    Right,
}

/// Vertical alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignY {
    /// Align to the top edge.
    #[default]
    Top,
    /// Align to the vertical center.
    Center,
    /// Align to the bottom edge.
    Bottom,
}

/// Layout configuration for a [`crate::Node`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Layout {
    /// Width and height rules.
    pub sizing: Sizing,
    /// Insets applied before placing children.
    pub padding: Padding,
    /// Space between normal-flow children.
    pub gap: f32,
    /// Main axis direction.
    pub direction: Direction,
    /// Horizontal alignment for children.
    pub align_x: AlignX,
    /// Vertical alignment for children.
    pub align_y: AlignY,
}

/// Normalized anchor point inside a rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anchor {
    /// Horizontal anchor, where `0` is left and `1` is right.
    pub x: f32,
    /// Vertical anchor, where `0` is top and `1` is bottom.
    pub y: f32,
}

impl Anchor {
    /// Top-left anchor.
    pub const TOP_LEFT: Self = Self { x: 0.0, y: 0.0 };
    /// Center anchor.
    pub const CENTER: Self = Self { x: 0.5, y: 0.5 };
    /// Bottom-right anchor.
    pub const BOTTOM_RIGHT: Self = Self { x: 1.0, y: 1.0 };
}

/// Target rectangle used by floating layout.
#[derive(Debug, Clone, PartialEq)]
pub enum AttachTo {
    /// Attach to the parent element.
    Parent,
    /// Attach to the root viewport.
    Root,
    /// Attach to another element by id.
    Element(String),
}

/// Hit-test behavior for floating nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointerCapture {
    /// Floating node participates in hit testing.
    #[default]
    Capture,
    /// Pointer events pass through to elements behind it.
    PassThrough,
}

/// Configuration for removing a child from normal flow and positioning it over another rectangle.
#[derive(Debug, Clone, PartialEq)]
pub struct Floating {
    /// Target rectangle to attach to.
    pub attach_to: AttachTo,
    /// Anchor inside the floating element.
    pub element_anchor: Anchor,
    /// Anchor inside the target rectangle.
    pub target_anchor: Anchor,
    /// Additional offset applied after anchor positioning.
    pub offset: Vector,
    /// Draw order among sibling floating nodes.
    pub z_index: i16,
    /// Whether the floating node captures pointer hits.
    pub pointer_capture: PointerCapture,
    /// Whether inherited clipping from the parent should still apply.
    pub clip_to_parent: bool,
}

impl Floating {
    /// Creates a floating config attached to the parent top-left corner.
    #[must_use]
    pub fn parent() -> Self {
        Self {
            attach_to: AttachTo::Parent,
            element_anchor: Anchor::TOP_LEFT,
            target_anchor: Anchor::TOP_LEFT,
            offset: Vector::ZERO,
            z_index: 0,
            pointer_capture: PointerCapture::Capture,
            clip_to_parent: false,
        }
    }
}

/// Border paint and per-edge width.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Border {
    /// Border color.
    pub color: Color,
    /// Per-edge border width.
    pub width: Padding,
    /// Per-edge border radius.
    pub radius: Radius,
}

/// Text rendering and measurement style.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    /// Text color emitted in render commands.
    pub color: Color,
    /// Caller-defined font/style id.
    pub font_id: u16,
    /// Font size in logical pixels.
    pub font_size: f32,
    /// Line height in logical pixels. `0` means use `font_size`.
    pub line_height: f32,
    /// Extra spacing between letters, passed through to measurement keys.
    pub letter_spacing: f32,
    /// Wrapping policy.
    pub wrap: TextWrap,
    /// Per-line horizontal alignment.
    pub align: TextAlign,
}

/// Text wrapping policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextWrap {
    /// Wrap at whitespace.
    #[default]
    Words,
    /// Split only at newline characters.
    Newlines,
    /// Do not wrap and replace newlines with spaces.
    None,
}

/// Horizontal text alignment inside the text node bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    /// Align text to the left.
    #[default]
    Left,
    /// Center each rendered line.
    Center,
    /// Align text to the right.
    Right,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            color: Color::rgba(0.0, 0.0, 0.0, 255.0),
            font_id: 0,
            font_size: 16.0,
            line_height: 0.0,
            letter_spacing: 0.0,
            wrap: TextWrap::Words,
            align: TextAlign::Left,
        }
    }
}
use crate::{
    Radius,
    geometry::{Color, Padding, Vector},
};
