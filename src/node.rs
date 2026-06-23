/// Semantic content stored by a [`Node`].
#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    /// A layout-only container.
    Container,
    /// A text node measured and rendered with a [`TextStyle`].
    Text {
        /// Text contents.
        text: String,
        /// Text style.
        style: TextStyle,
    },
    /// An image placeholder identified by an application-defined handle.
    Image(u64),
    /// A custom renderable payload identified by an application-defined handle.
    Custom(u64),
}

/// A layout tree node.
///
/// Nodes are immutable builder values: methods take and return `Self` so trees
/// can be composed fluently.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Optional string id used for queries and hit testing.
    pub id: Option<String>,
    /// Optional stable id metadata.
    pub element_id: Option<ElementId>,
    /// Layout configuration.
    pub layout: Layout,
    /// Background rectangle color.
    pub background: Color,
    /// Border paint and widths.
    pub border: Border,
    /// Optional aspect ratio used when one axis is fit-sized.
    pub aspect_ratio: Option<f32>,
    /// Clip children horizontally.
    pub clip_x: bool,
    /// Clip children vertically.
    pub clip_y: bool,
    /// Enable horizontal scrolling.
    pub scroll_x: bool,
    /// Enable vertical scrolling.
    pub scroll_y: bool,
    /// Optional overlay color emitted around child commands.
    pub overlay: Option<Color>,
    /// Optional application-owned custom command emitted for this node bounds.
    pub custom: Option<u64>,
    /// Optional floating layout configuration.
    pub floating: Option<Floating>,
    /// Child nodes.
    pub children: Vec<Node>,
    /// Node content kind.
    pub kind: NodeKind,
}

impl Node {
    /// Creates an empty container node.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a text node.
    pub fn text(text: impl Into<String>, style: TextStyle) -> Self {
        Self {
            kind: NodeKind::Text {
                text: text.into(),
                style,
            },
            ..Self::default()
        }
    }

    /// Creates a custom render node.
    #[must_use]
    pub fn custom(value: u64) -> Self {
        Self {
            kind: NodeKind::Custom(value),
            ..Self::default()
        }
    }

    /// Creates an image render node.
    #[must_use]
    pub fn image(value: u64) -> Self {
        Self {
            kind: NodeKind::Image(value),
            ..Self::default()
        }
    }

    /// Assigns a string id and matching stable [`ElementId`].
    #[must_use]
    pub fn id(mut self, id: impl Into<String>) -> Self {
        let id = id.into();
        self.element_id = Some(ElementId::new(id.clone()));
        self.id = Some(id);
        self
    }

    /// Assigns an explicit stable id.
    #[must_use]
    pub fn element_id(mut self, id: ElementId) -> Self {
        self.id = Some(id.label.clone());
        self.element_id = Some(id);
        self
    }

    /// Sets layout configuration.
    #[must_use]
    pub fn layout(mut self, layout: Layout) -> Self {
        self.layout = layout;
        self
    }

    /// Sets background color.
    #[must_use]
    pub fn background(mut self, color: Color) -> Self {
        self.background = color;
        self
    }

    /// Sets background corner radius.
    #[must_use]
    pub fn radius(mut self, radius: Radius) -> Self {
        self.border.radius = radius;
        self
    }

    /// Sets an aspect ratio for fit sizing.
    #[must_use]
    pub fn aspect_ratio(mut self, ratio: f32) -> Self {
        self.aspect_ratio = (ratio > 0.0).then_some(ratio);
        self
    }

    /// Enables clipping on each axis.
    #[must_use]
    pub fn clip(mut self, x: bool, y: bool) -> Self {
        self.clip_x = x;
        self.clip_y = y;
        self
    }

    /// Enables scrolling on each axis.
    #[must_use]
    pub fn scroll(mut self, x: bool, y: bool) -> Self {
        self.scroll_x = x;
        self.scroll_y = y;
        self
    }

    /// Emits an overlay around child commands.
    #[must_use]
    pub fn overlay(mut self, color: Color) -> Self {
        self.overlay = Some(color);
        self
    }

    /// Emits an application-owned custom command for this node bounds.
    #[must_use]
    pub fn custom_command(mut self, value: u64) -> Self {
        self.custom = Some(value);
        self
    }

    /// Removes this node from normal flow and positions it as floating content.
    #[must_use]
    pub fn floating(mut self, floating: Floating) -> Self {
        self.floating = Some(floating);
        self
    }

    /// Appends a child node.
    #[must_use]
    pub fn child(mut self, child: Node) -> Self {
        self.children.push(child);
        self
    }
}

impl Default for Node {
    fn default() -> Self {
        Self {
            id: None,
            element_id: None,
            layout: Layout::default(),
            background: Color::TRANSPARENT,
            border: Border::default(),
            aspect_ratio: None,
            clip_x: false,
            clip_y: false,
            scroll_x: false,
            scroll_y: false,
            overlay: None,
            custom: None,
            floating: None,
            children: Vec::new(),
            kind: NodeKind::Container,
        }
    }
}
use crate::geometry::{Color, Radius};
use crate::id::ElementId;
use crate::style::{Border, Floating, Layout, TextStyle};
