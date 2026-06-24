//! Rlay is an independent, safe Rust rewrite of the Clay layout engine with
//! additional functionality such as touchscreen support. It does not compile,
//! link to or depend on the original C implementation.
//!
//! The crate turns a tree of [`Node`] values into stable render commands,
//! element bounds, scroll state and pointer hit information. Rendering is left
//! to the host application: Rlay only computes geometry and emits an ordered
//! command stream.
//!
//! Rlay is intentionally headless. It does not open windows, allocate GPU
//! resources, shape fonts, draw text or own widgets. Your application supplies
//! text measurement, feeds pointer state into [`InputState`], and translates
//! [`RenderCommand`] values into whatever renderer it already uses.
//!
//! # Concepts
//!
//! - [`Engine`] stores frame-to-frame state such as scroll offsets, touch
//!   phases, scroll momentum, transitions and text measurement cache entries.
//! - [`Node`] is a frame-local tree input. A node may be a container, text,
//!   image or custom element.
//! - [`Layout`] controls sizing, padding, gaps, direction and alignment.
//! - [`LayoutResult`] is the output for one frame: render commands, element
//!   bounds, scroll containers, pointer hits and recoverable errors.
//! - [`RenderCommand`] is a renderer-facing instruction. Commands are emitted
//!   in paint order.
//!
//! # Frame Flow
//!
//! A typical application frame looks like this:
//!
//! 1. Update input with [`Engine::input_mut`].
//! 2. Build the [`Node`] tree for this frame.
//! 3. Call [`Engine::layout`] with delta time in seconds.
//! 4. Draw [`LayoutResult::commands`] with your renderer.
//! 5. Query [`LayoutResult::element`], [`LayoutResult::pointer_over`] or
//!    [`LayoutResult::scroll_container`] for interaction state.
//!
//! # Basic usage
//!
//! ```
//! use rlay::{Engine, Layout, Node, Size, Sizing};
//!
//! let mut engine = Engine::new(|text, style| {
//!     Size::new(text.chars().count() as f32 * style.font_size, style.font_size)
//! });
//!
//! let root = Node::new().child(Node::new().id("button").layout(Layout {
//!     sizing: Sizing::fixed(120.0, 32.0),
//!     ..Layout::default()
//! }));
//!
//! let result = engine.layout(&root, Size::new(320.0, 240.0), 0.0);
//! assert_eq!(result.element("button").unwrap().bounds.width, 120.0);
//! ```
//!
//! # Layout Model
//!
//! Rlay lays children along a main axis. [`Direction::Row`] places children
//! horizontally; [`Direction::Column`] places them vertically. Each axis can be:
//!
//! - [`AxisSize::Fixed`] for exact sizes.
//! - [`AxisSize::Fit`] for measured content.
//! - [`AxisSize::Grow`] for remaining space.
//! - [`AxisSize::Percent`] for a parent-relative fraction.
//!
//! ```
//! use rlay::{AxisSize, Direction, Layout, Node, Size, Sizing};
//!
//! let root = Node::new()
//!     .layout(Layout {
//!         direction: Direction::Row,
//!         gap: 8.0,
//!         ..Layout::default()
//!     })
//!     .child(Node::new().id("fixed").layout(Layout {
//!         sizing: Sizing::fixed(80.0, 24.0),
//!         ..Layout::default()
//!     }))
//!     .child(Node::new().id("fill").layout(Layout {
//!         sizing: Sizing {
//!             width: AxisSize::GROW,
//!             height: AxisSize::fixed(24.0),
//!         },
//!         ..Layout::default()
//!     }));
//! ```
//!
//! # Rendering
//!
//! Rlay does not draw. It emits commands such as [`CommandKind::Rectangle`],
//! [`CommandKind::Text`], [`CommandKind::Image`], [`CommandKind::ClipStart`] and
//! [`CommandKind::Custom`]. A renderer usually matches on [`CommandKind`] and
//! draws using the command bounds.
//!
//! ```
//! use rlay::{CommandKind, LayoutResult};
//!
//! fn count_text_commands(result: &LayoutResult) -> usize {
//!     result
//!         .commands
//!         .iter()
//!         .filter(|command| matches!(command.kind, CommandKind::Text { .. }))
//!         .count()
//! }
//! ```
//!
//! # Text Measurement
//!
//! Text sizing is host-defined. [`Engine::new`] receives a callback so the same
//! layout engine can be used with Skia, cosmic-text, platform APIs, game
//! engines or tests. The callback must return logical-pixel dimensions for a
//! string and [`TextStyle`].
//!
//! Rlay caches measurements by text, font id, font size, line height and letter
//! spacing. Use [`Engine::reset_measure_text_cache`] if your font backend or
//! DPI scale changes.
//!
//! # Input And Hit Testing
//!
//! [`InputState`] stores mouse and touch contacts. After layout, pointer hits
//! are available in [`LayoutResult::pointers`] and helper methods such as
//! [`LayoutResult::pointer_over`].
//!
//! ```
//! use rlay::{Engine, Node, Point, Size};
//!
//! let mut engine = Engine::new(|_, style| Size::new(0.0, style.font_size));
//! engine.input_mut().set_mouse_position(Point::new(10.0, 10.0));
//!
//! let result = engine.layout(&Node::new().id("root"), Size::new(100.0, 100.0), 0.0);
//! assert!(result.pointer_over("root"));
//! ```
//!
//! # Scroll And Touch
//!
//! Nodes can become scroll containers with [`Node::scroll`]. Scroll offsets are
//! persistent engine state and are exposed through [`ScrollData`]. Touch drag
//! scrolling averages active fingers so a two-finger drag does not scroll twice
//! as fast as a one-finger drag. Pinch state is available with
//! [`InputState::pinch`].
//!
//! # Transitions
//!
//! Nodes with stable ids can animate selected geometry and paint properties
//! between frames.
//!
//! ```
//! use rlay::{Engine, Node, Size, Transition, TransitionProperties};
//!
//! let mut engine = Engine::new(|_, style| Size::new(0.0, style.font_size));
//! let panel = Node::new().id("panel").transition(Transition::ease_out(
//!     0.2,
//!     TransitionProperties::WIDTH | TransitionProperties::BACKGROUND_COLOR,
//! ));
//! let result = engine.layout(&panel, Size::new(100.0, 40.0), 1.0 / 60.0);
//! assert!(result.errors.is_empty());
//! ```
//!
//! # Floating And Overlays
//!
//! A node with [`Node::floating`] is removed from normal flow and positioned
//! relative to its parent, the root, or another element id. Floating nodes can
//! capture pointer hits or pass them through with [`PointerCapture`]. Overlays
//! are emitted as paired [`CommandKind::OverlayStart`] and
//! [`CommandKind::OverlayEnd`] commands.
//!
//! # Direct Tree API And Builder API
//!
//! Rlay has two APIs for producing the same per-frame layout input:
//!
//! - The direct tree API: construct [`Node`] values yourself and pass the root
//!   to [`Engine::layout`].
//! - The builder API: call [`Engine::begin`] to get a [`Frame`], push nodes
//!   through a temporary stack, and call [`Frame::end`] to produce and lay out
//!   the root tree.
//!
//! These are API styles, not different UI lifetime models. The direct tree API
//! is stateless with respect to tree construction. The builder API is stateful
//! while the frame is open. Both still rebuild the UI tree for each frame and
//! both end in the same layout pipeline.
//!
//! ```
//! use rlay::{Engine, Node, Size};
//!
//! let mut engine = Engine::new(|_, style| Size::new(0.0, style.font_size));
//! let mut frame = engine.begin(Size::new(200.0, 100.0));
//! frame.open(Node::new().id("panel"));
//! frame.child(Node::new().id("button"));
//! frame.close().unwrap();
//! let result = frame.end(0.0).unwrap();
//! assert!(result.element("button").is_some());
//! ```
//!
//! # Error Handling
//!
//! Most layout work is infallible. Recoverable issues are accumulated in
//! [`LayoutResult::errors`] as [`LayoutError`] values, for example when optional
//! capacity limits are exceeded. Builder structural mistakes are returned from
//! [`Frame::close`] and [`Frame::end`].
//!
//! Rlay deliberately does not own a renderer, font system or widget toolkit.
//! Applications provide text measurement through [`Engine::new`], submit input
//! through [`InputState`], and draw the returned [`RenderCommand`] values with
//! their own graphics backend.

#![warn(missing_docs)]
#![warn(clippy::pedantic)]
#![warn(clippy::correctness)]
#![warn(clippy::perf)]
#![warn(clippy::style)]
#![warn(clippy::suspicious)]
#![allow(clippy::cast_precision_loss)]

mod engine;
mod frame;
mod geometry;
mod id;
mod input;
mod node;
mod result;
mod scroll;
mod style;
mod text;
mod transition;

pub use engine::{Engine, LayoutError, MeasureText};
pub use frame::Frame;
pub use geometry::{Color, Padding, Point, Radius, Rect, Size, Vector};
pub use id::ElementId;
pub use input::{
    InputState, MouseButton, PinchGesture, Pointer, PointerGesture, PointerHit, PointerId,
    PointerPhase, TouchPhase,
};
pub use node::{Node, NodeKind};
pub use result::{CommandKind, ElementData, LayoutResult, RenderCommand, ScrollData};
pub use style::{
    AlignX, AlignY, Anchor, AttachTo, AxisSize, Border, Direction, Floating, Layout,
    PointerCapture, Sizing, TextAlign, TextStyle, TextWrap,
};
pub use text::TextSelection;
pub use transition::{
    Transition, TransitionArgs, TransitionEnter, TransitionEnterTrigger, TransitionExit,
    TransitionExitOrdering, TransitionExitTrigger, TransitionFrame, TransitionInteraction,
    TransitionProperties, TransitionState, TransitionValues, ease_out,
};

#[cfg(test)]
mod tests;
