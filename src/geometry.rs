/// Two-dimensional extent in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Size {
    /// Horizontal extent.
    pub width: f32,
    /// Vertical extent.
    pub height: f32,
}

impl Size {
    /// Empty size.
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    /// Creates a size from width and height.
    #[must_use]
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

impl From<(f32, f32)> for Size {
    fn from((width, height): (f32, f32)) -> Self {
        Self::new(width, height)
    }
}

/// A position in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Point {
    /// Horizontal coordinate.
    pub x: f32,
    /// Vertical coordinate.
    pub y: f32,
}

/// A two-dimensional offset or velocity.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vector {
    /// Horizontal component.
    pub x: f32,
    /// Vertical component.
    pub y: f32,
}

impl Vector {
    /// Zero offset.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// Creates a vector from horizontal and vertical components.
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl From<(f32, f32)> for Vector {
    fn from((x, y): (f32, f32)) -> Self {
        Self::new(x, y)
    }
}

impl Point {
    /// Creates a point from coordinates.
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Axis-aligned rectangle in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Rectangle width.
    pub width: f32,
    /// Rectangle height.
    pub height: f32,
}

impl Rect {
    /// Creates a rectangle from origin and size.
    #[must_use]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Returns whether `point` is inside the rectangle.
    #[must_use]
    pub fn contains(self, point: Point) -> bool {
        point.x >= self.x
            && point.y >= self.y
            && point.x < self.x + self.width
            && point.y < self.y + self.height
    }

    /// Returns the overlapping rectangle, if the rectangles overlap.
    #[must_use]
    pub fn intersection(self, other: Self) -> Option<Self> {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.width).min(other.x + other.width);
        let y2 = (self.y + self.height).min(other.y + other.height);
        (x2 > x1 && y2 > y1).then(|| Self::new(x1, y1, x2 - x1, y2 - y1))
    }
}

/// RGBA color.
///
/// Rlay stores color values as `f32` and does not enforce a range. Renderers may
/// choose the color convention they need; examples in this crate use `0..=255`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Color {
    /// Red channel.
    pub r: f32,
    /// Green channel.
    pub g: f32,
    /// Blue channel.
    pub b: f32,
    /// Alpha channel.
    pub a: f32,
}

impl Color {
    /// Fully transparent black.
    pub const TRANSPARENT: Self = Self::rgba(0.0, 0.0, 0.0, 0.0);

    /// Creates an opaque RGB color.
    #[must_use]
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::rgba(r, g, b, 255.0)
    }

    /// Creates a color from RGBA channel values.
    #[must_use]
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Returns true when the alpha channel is greater than zero.
    #[must_use]
    pub fn is_visible(self) -> bool {
        self.a > 0.0
    }
}

impl From<(u8, u8, u8, u8)> for Color {
    fn from((r, g, b, a): (u8, u8, u8, u8)) -> Self {
        Self::rgba(f32::from(r), f32::from(g), f32::from(b), f32::from(a))
    }
}

impl From<(u8, u8, u8)> for Color {
    fn from((r, g, b): (u8, u8, u8)) -> Self {
        Self::rgb(f32::from(r), f32::from(g), f32::from(b))
    }
}

impl From<(f32, f32, f32, f32)> for Color {
    fn from((r, g, b, a): (f32, f32, f32, f32)) -> Self {
        Self::rgba(r, g, b, a)
    }
}

impl From<(f32, f32, f32)> for Color {
    fn from((r, g, b): (f32, f32, f32)) -> Self {
        Self::rgb(r, g, b)
    }
}

/// Corner radii for a rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Radius {
    /// Top-left radius.
    pub top_left: f32,
    /// Top-right radius.
    pub top_right: f32,
    /// Bottom-left radius.
    pub bottom_left: f32,
    /// Bottom-right radius.
    pub bottom_right: f32,
}

impl Radius {
    /// Creates equal radii for all corners.
    #[must_use]
    pub const fn all(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_left: radius,
            bottom_right: radius,
        }
    }
}
impl From<(f32, f32, f32, f32)> for Radius {
    fn from(value: (f32, f32, f32, f32)) -> Self {
        Self {
            top_left: value.0,
            top_right: value.1,
            bottom_left: value.2,
            bottom_right: value.3,
        }
    }
}
impl From<(f64, f64, f64, f64)> for Radius {
    #[allow(clippy::cast_possible_truncation)]
    fn from(value: (f64, f64, f64, f64)) -> Self {
        Self {
            top_left: value.0 as _,
            top_right: value.1 as _,
            bottom_left: value.2 as _,
            bottom_right: value.3 as _,
        }
    }
}

/// Insets applied around a rectangle or content box.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Padding {
    /// Left inset.
    pub left: f32,
    /// Right inset.
    pub right: f32,
    /// Top inset.
    pub top: f32,
    /// Bottom inset.
    pub bottom: f32,
}

impl Padding {
    /// Creates padding from individual edges.
    #[must_use]
    pub const fn new(left: f32, right: f32, top: f32, bottom: f32) -> Self {
        Self {
            left,
            right,
            top,
            bottom,
        }
    }

    /// Creates equal padding on every side.
    #[must_use]
    pub const fn all(value: f32) -> Self {
        Self {
            left: value,
            right: value,
            top: value,
            bottom: value,
        }
    }

    pub(crate) fn horizontal(self) -> f32 {
        self.left + self.right
    }

    pub(crate) fn vertical(self) -> f32 {
        self.top + self.bottom
    }
}
