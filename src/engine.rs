use crate::frame::Frame;
use crate::geometry::{Color, Point, Radius, Rect, Size, Vector};
use crate::input::{InputState, PointerHit};
use crate::node::{Node, NodeKind};
use crate::result::{
    ClipRegion, CommandKind, ElementData, HitEntry, LayoutResult, RenderCommand, ScrollData,
};
use crate::scroll::ScrollState;
use crate::style::{
    AlignX, AlignY, AttachTo, AxisSize, Direction, Layout, PointerCapture, Sizing, TextAlign,
    TextStyle, TextWrap,
};
use crate::text::{TextSelection, char_index_to_byte, ease_out, main_axis, resolved_line_height};
use std::collections::HashMap;

/// Text measurement callback used by the layout engine.
///
/// The callback receives text and style, and returns the measured size in
/// logical pixels.
pub type MeasureText = dyn Fn(&str, &TextStyle) -> Size;

/// Stateful layout engine.
///
/// `Engine` owns transient state that must persist across frames: input phases,
/// scroll offsets, momentum, transitions and the text measurement cache.
#[allow(clippy::struct_excessive_bools)]
pub struct Engine {
    measure_text: Box<MeasureText>,
    measure_cache: HashMap<TextMeasureKey, Size>,
    input: InputState,
    scroll: ScrollState,
    transitions: HashMap<String, Rect>,
    exiting_commands: HashMap<String, Vec<RenderCommand>>,
    culling: bool,
    debug: bool,
    max_elements: Option<usize>,
    max_commands: Option<usize>,
    max_measure_cache: Option<usize>,
    measure_cache_exceeded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TextMeasureKey {
    text: String,
    font_id: u16,
    font_size: u32,
    line_height: u32,
    letter_spacing: u32,
}

#[derive(Clone, Copy)]
struct IntrinsicSize {
    preferred: Size,
    minimum: Size,
}

/// Recoverable errors reported in [`LayoutResult`](crate::LayoutResult).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    /// `Frame::close` was called when no child frame was open.
    UnbalancedClose,
    /// `Frame::end` was called while child frames were still open.
    UnclosedElements,
    /// The configured element capacity was exceeded.
    ElementsCapacityExceeded,
    /// The configured render command capacity was exceeded.
    CommandsCapacityExceeded,
    /// The configured text measurement cache capacity was exceeded.
    TextMeasurementCapacityExceeded,
}

impl Engine {
    /// Creates an engine with a text measurement callback.
    pub fn new(measure_text: impl Fn(&str, &TextStyle) -> Size + 'static) -> Self {
        Self {
            measure_text: Box::new(measure_text),
            measure_cache: HashMap::new(),
            input: InputState::default(),
            scroll: ScrollState::default(),
            transitions: HashMap::new(),
            exiting_commands: HashMap::new(),
            culling: false,
            debug: false,
            max_elements: None,
            max_commands: None,
            max_measure_cache: None,
            measure_cache_exceeded: false,
        }
    }

    /// Returns mutable access to input state for the next frame.
    pub fn input_mut(&mut self) -> &mut InputState {
        &mut self.input
    }

    /// Returns access to input state for the next frame.
    #[must_use]
    pub fn input(&self) -> &InputState {
        &self.input
    }

    /// Enables or disables command culling outside the viewport.
    pub fn set_culling(&mut self, enabled: bool) {
        self.culling = enabled;
    }

    /// Enables or disables debug render commands.
    pub fn set_debug(&mut self, enabled: bool) {
        self.debug = enabled;
    }

    /// Sets an optional maximum number of tracked elements.
    pub fn set_max_elements(&mut self, max: Option<usize>) {
        self.max_elements = max;
    }

    /// Sets an optional maximum number of render commands.
    pub fn set_max_commands(&mut self, max: Option<usize>) {
        self.max_commands = max;
    }

    /// Sets an optional maximum number of cached text measurements.
    pub fn set_max_measure_text_cache_entries(&mut self, max: Option<usize>) {
        self.max_measure_cache = max;
    }

    /// Enables or disables touch drag scrolling.
    pub fn set_drag_scrolling(&mut self, enabled: bool) {
        self.scroll.set_drag_scrolling(enabled);
    }

    /// Sets a persistent scroll offset for an element id.
    pub fn set_scroll_offset(&mut self, id: impl Into<String>, offset: Vector) {
        self.scroll.set_offset(id, offset);
    }

    /// Sets scroll momentum velocity for an element id.
    pub fn set_scroll_momentum(&mut self, id: impl Into<String>, velocity: Vector) {
        self.scroll.set_momentum(id, velocity);
    }

    /// Overrides the bounds used for an element id in the next layout pass.
    pub fn set_transition_bounds(&mut self, id: impl Into<String>, bounds: Rect) {
        self.transitions.insert(id.into(), bounds);
    }

    /// Computes eased bounds and stores them as a transition override.
    pub fn transition_bounds(
        &mut self,
        id: impl Into<String>,
        from: Rect,
        to: Rect,
        elapsed: f32,
        duration: f32,
    ) {
        self.transitions.insert(
            id.into(),
            Rect::new(
                ease_out(from.x, to.x, elapsed, duration),
                ease_out(from.y, to.y, elapsed, duration),
                ease_out(from.width, to.width, elapsed, duration),
                ease_out(from.height, to.height, elapsed, duration),
            ),
        );
    }

    /// Stores commands to emit when an element disappears.
    pub fn transition_exit_commands(
        &mut self,
        id: impl Into<String>,
        commands: Vec<RenderCommand>,
    ) {
        self.exiting_commands.insert(id.into(), commands);
    }

    /// Sets an external scroll query callback.
    pub fn set_query_scroll_offset(&mut self, query: impl Fn(&str) -> Vector + 'static) {
        self.scroll.set_query_offset(query);
    }

    /// Returns the current scroll offset for an element id.
    #[must_use]
    pub fn scroll_offset(&self, id: &str) -> Vector {
        self.scroll.offset(id)
    }

    /// Applies pointer-driven scroll using an already computed layout.
    ///
    /// This is useful for event-driven renderers that need input from the
    /// current event to affect the frame being rendered.
    pub fn apply_input_scroll(&mut self, result: &LayoutResult) {
        self.scroll.apply_input_scroll(&mut self.input, result);
    }

    /// Clears the text measurement cache.
    pub fn reset_measure_text_cache(&mut self) {
        self.measure_cache.clear();
    }

    /// Returns the number of cached text measurements.
    #[must_use]
    pub fn measure_text_cache_len(&self) -> usize {
        self.measure_cache.len()
    }

    /// Begins an immediate-mode frame.
    pub fn begin(&mut self, size: Size) -> Frame<'_> {
        Frame {
            engine: self,
            size,
            stack: vec![Node::new().layout(Layout {
                sizing: Sizing::fixed(size.width, size.height),
                ..Layout::default()
            })],
        }
    }

    /// Lays out a retained [`Node`] tree.
    pub fn layout(&mut self, root: &Node, size: Size) -> LayoutResult {
        let mut result = LayoutResult::default();
        let root_bounds = Rect::new(0.0, 0.0, size.width, size.height);
        self.layout_node(root, root_bounds, root_bounds, &mut result);
        if self
            .max_elements
            .is_some_and(|max_elements| result.elements.len() > max_elements)
        {
            result.errors.push(LayoutError::ElementsCapacityExceeded);
        }
        if self
            .max_commands
            .is_some_and(|max_commands| result.commands.len() > max_commands)
        {
            result.errors.push(LayoutError::CommandsCapacityExceeded);
        }
        if self.measure_cache_exceeded {
            result
                .errors
                .push(LayoutError::TextMeasurementCapacityExceeded);
            self.measure_cache_exceeded = false;
        }
        for (id, commands) in &self.exiting_commands {
            if !result.elements.contains_key(id) {
                result.commands.extend(commands.iter().cloned());
            }
        }
        result.pointers = self
            .input
            .pointers()
            .map(|pointer| PointerHit {
                pointer_id: pointer.id,
                position: pointer.position,
                phase: pointer.phase,
                element_id: self
                    .input
                    .pointer_capture(pointer.id)
                    .map(str::to_owned)
                    .or_else(|| Engine::hit_test(&result, pointer.position).map(str::to_owned)),
                mouse_button: pointer.mouse_button,
                gesture: pointer.gesture,
            })
            .collect();
        self.scroll.finish_layout_frame(&mut self.input, &result);
        self.input.end_frame();
        if self.debug {
            result.commands.push(RenderCommand {
                id: Some("__rlay_debug_panel".into()),
                bounds: Rect::new(root_bounds.x, root_bounds.y, 220.0, 48.0),
                kind: CommandKind::Rectangle {
                    color: Color::rgba(0.0, 0.0, 0.0, 180.0),
                    radius: Radius::all(4.0),
                },
            });
            result.commands.push(RenderCommand {
                id: Some("__rlay_debug_text".into()),
                bounds: Rect::new(root_bounds.x + 8.0, root_bounds.y + 8.0, 204.0, 16.0),
                kind: CommandKind::Text {
                    text: format!(
                        "elements: {} commands: {}",
                        result.elements.len(),
                        result.commands.len()
                    ),
                    style: TextStyle {
                        color: Color::rgba(255.0, 255.0, 255.0, 255.0),
                        font_size: 12.0,
                        ..TextStyle::default()
                    },
                },
            });
            result.commands.push(RenderCommand {
                id: Some("__rlay_debug".into()),
                bounds: root_bounds,
                kind: CommandKind::DebugOverlay {
                    elements: result.elements.len(),
                    commands: result.commands.len(),
                },
            });
        }
        result
    }

    /// Returns the topmost element id containing `point`.
    #[must_use]
    pub fn hit_test(result: &LayoutResult, point: Point) -> Option<&str> {
        result
            .hit_order
            .iter()
            .rev()
            .find(|entry| entry.bounds.contains(point))
            .map(|entry| entry.id.as_str())
    }

    fn layout_node(
        &mut self,
        node: &Node,
        bounds: Rect,
        viewport: Rect,
        result: &mut LayoutResult,
    ) {
        self.layout_node_clipped(node, bounds, viewport, ClipRegion::default(), result);
    }

    #[allow(clippy::too_many_lines)]
    fn layout_node_clipped(
        &mut self,
        node: &Node,
        bounds: Rect,
        viewport: Rect,
        parent_clip: ClipRegion,
        result: &mut LayoutResult,
    ) {
        let clip_x = node.clip_x || node.scroll_x;
        let clip_y = node.clip_y || node.scroll_y;
        let current_clip = parent_clip.add(bounds, clip_x, clip_y);
        let hit_bounds = parent_clip.apply(bounds);
        let culled = self.culling && bounds.intersection(viewport).is_none();
        let bounds = node
            .id
            .as_deref()
            .and_then(|id| self.transitions.get(id).copied())
            .unwrap_or(bounds);

        let hit_testable = node
            .floating
            .as_ref()
            .is_none_or(|floating| floating.pointer_capture == PointerCapture::Capture);

        if let Some(id) = &node.id {
            result.elements.insert(
                id.clone(),
                ElementData {
                    bounds,
                    element_id: node.element_id.clone(),
                },
            );
            if hit_testable && let Some(bounds) = hit_bounds {
                result.hit_order.push(HitEntry {
                    id: id.clone(),
                    bounds,
                });
            }
        }

        if node.background.is_visible() && !culled {
            result.commands.push(RenderCommand {
                id: node.id.clone(),
                bounds,
                kind: CommandKind::Rectangle {
                    color: node.background,
                    radius: node.border.radius,
                },
            });
        }

        if (clip_x || clip_y) && !culled {
            result.commands.push(RenderCommand {
                id: node.id.clone(),
                bounds,
                kind: CommandKind::ClipStart {
                    x: clip_x,
                    y: clip_y,
                },
            });
        }

        if let Some(overlay) = node.overlay.filter(|_| !culled) {
            result.commands.push(RenderCommand {
                id: node.id.clone(),
                bounds,
                kind: CommandKind::OverlayStart(overlay),
            });
        }

        if let Some(value) = node.custom.filter(|_| !culled) {
            result.commands.push(RenderCommand {
                id: node.id.clone(),
                bounds,
                kind: CommandKind::Custom(value, node.border.radius),
            });
        }

        match &node.kind {
            NodeKind::Container => {
                self.layout_children(node, bounds, viewport, current_clip, result);
            }
            NodeKind::Text { text, style } => {
                for (line, line_bounds) in self.text_render_lines(text, style, bounds) {
                    if culled {
                        continue;
                    }
                    result.commands.push(RenderCommand {
                        id: node.id.clone(),
                        bounds: line_bounds,
                        kind: CommandKind::Text {
                            text: line,
                            style: style.clone(),
                        },
                    });
                }
            }
            NodeKind::Image(value) => {
                if !culled {
                    result.commands.push(RenderCommand {
                        id: node.id.clone(),
                        bounds,
                        kind: CommandKind::Image(*value),
                    });
                }
            }
            NodeKind::Custom(value) => {
                if !culled {
                    result.commands.push(RenderCommand {
                        id: node.id.clone(),
                        bounds,
                        kind: CommandKind::Custom(*value, node.border.radius),
                    });
                }
            }
        }

        if !culled && (node.border.width.horizontal() > 0.0 || node.border.width.vertical() > 0.0) {
            result.commands.push(RenderCommand {
                id: node.id.clone(),
                bounds,
                kind: CommandKind::Border(node.border),
            });
        }

        if node.overlay.is_some() && !culled {
            result.commands.push(RenderCommand {
                id: node.id.clone(),
                bounds,
                kind: CommandKind::OverlayEnd,
            });
        }

        if (clip_x || clip_y) && !culled {
            result.commands.push(RenderCommand {
                id: node.id.clone(),
                bounds,
                kind: CommandKind::ClipEnd,
            });
        }
    }

    #[allow(clippy::too_many_lines)]
    fn layout_children(
        &mut self,
        node: &Node,
        bounds: Rect,
        viewport: Rect,
        clip: ClipRegion,
        result: &mut LayoutResult,
    ) {
        let content = Rect::new(
            bounds.x + node.layout.padding.left,
            bounds.y + node.layout.padding.top,
            (bounds.width - node.layout.padding.horizontal()).max(0.0),
            (bounds.height - node.layout.padding.vertical()).max(0.0),
        );

        let scroll_offset = node
            .id
            .as_deref()
            .map_or(Vector::ZERO, |id| self.scroll_offset(id));

        let main_available = match node.layout.direction {
            Direction::Row => content.width,
            Direction::Column => content.height,
        };
        let cross_available = match node.layout.direction {
            Direction::Row => content.height,
            Direction::Column => content.width,
        };

        let normal_children: Vec<_> = node
            .children
            .iter()
            .filter(|child| child.floating.is_none())
            .collect();
        let mut floating_children: Vec<_> = node
            .children
            .iter()
            .filter(|child| child.floating.is_some())
            .collect();
        floating_children.sort_by_key(|child| child.floating.as_ref().map_or(0, |f| f.z_index));

        let child_sizes = self.resolve_children_sizes(
            &normal_children,
            Size::new(content.width, content.height),
            node,
        );
        let used_main = node.layout.gap * normal_children.len().saturating_sub(1) as f32
            + child_sizes
                .iter()
                .map(|size| main_axis(*size, node.layout.direction))
                .sum::<f32>();
        let mut cursor = match node.layout.direction {
            Direction::Row => match node.layout.align_x {
                AlignX::Left => content.x - scroll_offset.x,
                AlignX::Center => {
                    content.x + (main_available - used_main).max(0.0) / 2.0 - scroll_offset.x
                }
                AlignX::Right => {
                    content.x + (main_available - used_main).max(0.0) - scroll_offset.x
                }
            },
            Direction::Column => match node.layout.align_y {
                AlignY::Top => content.y - scroll_offset.y,
                AlignY::Center => {
                    content.y + (main_available - used_main).max(0.0) / 2.0 - scroll_offset.y
                }
                AlignY::Bottom => {
                    content.y + (main_available - used_main).max(0.0) - scroll_offset.y
                }
            },
        };

        if let Some(id) = &node.id
            && (node.scroll_x || node.scroll_y)
        {
            let content_size = match node.layout.direction {
                Direction::Row => Size::new(
                    used_main + node.layout.padding.horizontal(),
                    cross_available + node.layout.padding.vertical(),
                ),
                Direction::Column => Size::new(
                    cross_available + node.layout.padding.horizontal(),
                    used_main + node.layout.padding.vertical(),
                ),
            };
            result.scroll_containers.insert(
                id.clone(),
                ScrollData {
                    bounds,
                    content_size,
                    offset: scroll_offset,
                    scroll_x: node.scroll_x,
                    scroll_y: node.scroll_y,
                },
            );
        }

        for (child, child_size) in normal_children.into_iter().zip(child_sizes) {
            let cross_offset = match node.layout.direction {
                Direction::Row => match node.layout.align_y {
                    AlignY::Top => 0.0,
                    AlignY::Center => (cross_available - child_size.height).max(0.0) / 2.0,
                    AlignY::Bottom => (cross_available - child_size.height).max(0.0),
                },
                Direction::Column => match node.layout.align_x {
                    AlignX::Left => 0.0,
                    AlignX::Center => (cross_available - child_size.width).max(0.0) / 2.0,
                    AlignX::Right => (cross_available - child_size.width).max(0.0),
                },
            };

            let child_bounds = match node.layout.direction {
                Direction::Row => Rect::new(
                    cursor,
                    content.y + cross_offset,
                    child_size.width,
                    child_size.height,
                ),
                Direction::Column => Rect::new(
                    content.x + cross_offset,
                    cursor,
                    child_size.width,
                    child_size.height,
                ),
            };

            self.layout_node_clipped(child, child_bounds, viewport, clip, result);
            cursor += main_axis(child_size, node.layout.direction) + node.layout.gap;
        }

        for child in floating_children {
            let floating = child.floating.as_ref().expect("filtered floating children");
            let fit = self.measure_node(child);
            let size = self.resolve_child_size(
                child,
                fit,
                bounds.width,
                bounds.height,
                None,
                Direction::Row,
            );
            let target = match &floating.attach_to {
                AttachTo::Parent => bounds,
                AttachTo::Root => viewport,
                AttachTo::Element(id) => {
                    result.element(id).map_or(bounds, |element| element.bounds)
                }
            };
            let child_bounds = Rect::new(
                target.x + target.width * floating.target_anchor.x
                    - size.width * floating.element_anchor.x
                    + floating.offset.x,
                target.y + target.height * floating.target_anchor.y
                    - size.height * floating.element_anchor.y
                    + floating.offset.y,
                size.width,
                size.height,
            );
            let floating_clip = if floating.clip_to_parent {
                clip
            } else {
                ClipRegion::default()
            };
            self.layout_node_clipped(child, child_bounds, viewport, floating_clip, result);
        }
    }

    fn resolve_child_size(
        &mut self,
        child: &Node,
        fit: Size,
        main_available: f32,
        cross_available: f32,
        main_size: Option<f32>,
        direction: Direction,
    ) -> Size {
        let width_available = match direction {
            Direction::Row => main_available,
            Direction::Column => cross_available,
        };
        let height_available = match direction {
            Direction::Row => cross_available,
            Direction::Column => main_available,
        };
        let mut width = child
            .layout
            .sizing
            .width
            .resolve(width_available, fit.width);
        let mut height = child
            .layout
            .sizing
            .height
            .resolve(height_available, fit.height);

        if let Some(main_size) = main_size {
            match direction {
                Direction::Row => width = main_size,
                Direction::Column => height = main_size,
            }
        }

        if let Some(ratio) = child.aspect_ratio {
            match (child.layout.sizing.width, child.layout.sizing.height) {
                (_, AxisSize::Fit { .. }) if width > 0.0 => height = width / ratio,
                (AxisSize::Fit { .. }, _) if height > 0.0 => width = height * ratio,
                _ => {}
            }
        }

        if let NodeKind::Text { text, style } = &child.kind
            && !matches!(
                child.layout.sizing.height,
                AxisSize::Fixed(_) | AxisSize::Grow { .. }
            )
        {
            height = self.text_layout_size(text, style, width).height;
        }

        Size::new(width, height)
    }

    fn measure_node(&mut self, node: &Node) -> Size {
        self.intrinsic_size(node).preferred
    }

    #[allow(clippy::too_many_lines)]
    fn intrinsic_size(&mut self, node: &Node) -> IntrinsicSize {
        match &node.kind {
            NodeKind::Text { text, style } => {
                let preferred_width = text
                    .split('\n')
                    .map(|line| self.measure_text_cached(line, style).width)
                    .fold(0.0, f32::max);
                let minimum_width = if style.wrap == TextWrap::Words {
                    text.split_whitespace()
                        .map(|word| self.measure_text_cached(word, style).width)
                        .fold(0.0, f32::max)
                } else {
                    preferred_width
                };
                let height = resolved_line_height(style);
                IntrinsicSize {
                    preferred: Size::new(preferred_width, height),
                    minimum: Size::new(minimum_width, height),
                }
            }
            NodeKind::Image(_) | NodeKind::Custom(_) => IntrinsicSize {
                preferred: Size::ZERO,
                minimum: Size::ZERO,
            },
            NodeKind::Container => {
                let mut preferred_main: f32 = 0.0;
                let mut preferred_cross: f32 = 0.0;
                let mut minimum_main: f32 = 0.0;
                let mut minimum_cross: f32 = 0.0;
                for (index, child) in node
                    .children
                    .iter()
                    .filter(|child| child.floating.is_none())
                    .enumerate()
                {
                    let size = self.intrinsic_size(child);
                    if index > 0 {
                        preferred_main += node.layout.gap;
                        minimum_main += node.layout.gap;
                    }
                    match node.layout.direction {
                        Direction::Row => {
                            preferred_main +=
                                intrinsic_axis(child.layout.sizing.width, size.preferred.width);
                            preferred_cross = preferred_cross.max(intrinsic_axis(
                                child.layout.sizing.height,
                                size.preferred.height,
                            ));
                            if !node.clip_x && !node.scroll_x {
                                minimum_main +=
                                    minimum_axis(child.layout.sizing.width, size.minimum.width);
                            }
                            if !node.clip_y && !node.scroll_y {
                                minimum_cross = minimum_cross.max(minimum_axis(
                                    child.layout.sizing.height,
                                    size.minimum.height,
                                ));
                            }
                        }
                        Direction::Column => {
                            preferred_main +=
                                intrinsic_axis(child.layout.sizing.height, size.preferred.height);
                            preferred_cross = preferred_cross.max(intrinsic_axis(
                                child.layout.sizing.width,
                                size.preferred.width,
                            ));
                            if !node.clip_y && !node.scroll_y {
                                minimum_main +=
                                    minimum_axis(child.layout.sizing.height, size.minimum.height);
                            }
                            if !node.clip_x && !node.scroll_x {
                                minimum_cross = minimum_cross.max(minimum_axis(
                                    child.layout.sizing.width,
                                    size.minimum.width,
                                ));
                            }
                        }
                    }
                }

                let (preferred, minimum) = match node.layout.direction {
                    Direction::Row => (
                        Size::new(
                            preferred_main + node.layout.padding.horizontal(),
                            preferred_cross + node.layout.padding.vertical(),
                        ),
                        Size::new(
                            minimum_main + node.layout.padding.horizontal(),
                            minimum_cross + node.layout.padding.vertical(),
                        ),
                    ),
                    Direction::Column => (
                        Size::new(
                            preferred_cross + node.layout.padding.horizontal(),
                            preferred_main + node.layout.padding.vertical(),
                        ),
                        Size::new(
                            minimum_cross + node.layout.padding.horizontal(),
                            minimum_main + node.layout.padding.vertical(),
                        ),
                    ),
                };
                IntrinsicSize { preferred, minimum }
            }
        }
    }

    fn resolve_children_sizes(
        &mut self,
        children: &[&Node],
        available: Size,
        parent: &Node,
    ) -> Vec<Size> {
        let intrinsic: Vec<_> = children
            .iter()
            .map(|child| self.intrinsic_size(child))
            .collect();
        let gap = parent.layout.gap * children.len().saturating_sub(1) as f32;
        let width_available = (available.width - gap).max(0.0);
        let mut widths: Vec<_> = children
            .iter()
            .zip(&intrinsic)
            .map(|(child, size)| match parent.layout.direction {
                Direction::Row => initial_axis(
                    child.layout.sizing.width,
                    size.preferred.width,
                    width_available,
                ),
                Direction::Column => cross_axis(
                    child.layout.sizing.width,
                    size.preferred.width,
                    size.minimum.width,
                    available.width,
                ),
            })
            .collect();

        if parent.layout.direction == Direction::Row {
            let minimums: Vec<_> = children
                .iter()
                .zip(&intrinsic)
                .map(|(child, size)| minimum_axis(child.layout.sizing.width, size.minimum.width))
                .collect();
            distribute_axis(
                children,
                &mut widths,
                &minimums,
                width_available,
                true,
                !parent.clip_x && !parent.scroll_x,
            );
        }

        let natural_heights: Vec<_> = children
            .iter()
            .zip(&widths)
            .map(|(child, width)| self.height_for_width(child, *width))
            .collect();
        let height_available = (available.height - gap).max(0.0);
        let mut heights: Vec<_> = children
            .iter()
            .zip(&intrinsic)
            .zip(&natural_heights)
            .map(|((child, size), natural)| match parent.layout.direction {
                Direction::Row => cross_axis(
                    child.layout.sizing.height,
                    *natural,
                    size.minimum.height,
                    available.height,
                ),
                Direction::Column => {
                    initial_axis(child.layout.sizing.height, *natural, height_available)
                }
            })
            .collect();

        if parent.layout.direction == Direction::Column {
            let minimums: Vec<_> = children
                .iter()
                .zip(&intrinsic)
                .map(|(child, size)| minimum_axis(child.layout.sizing.height, size.minimum.height))
                .collect();
            distribute_axis(
                children,
                &mut heights,
                &minimums,
                height_available,
                false,
                !parent.clip_y && !parent.scroll_y,
            );
        }

        children
            .iter()
            .zip(widths)
            .zip(heights)
            .map(|((child, mut width), mut height)| {
                if let Some(ratio) = child.aspect_ratio {
                    match (child.layout.sizing.width, child.layout.sizing.height) {
                        (_, AxisSize::Fit { .. }) if width > 0.0 => height = width / ratio,
                        (AxisSize::Fit { .. }, _) if height > 0.0 => width = height * ratio,
                        _ => {}
                    }
                }
                Size::new(width, height)
            })
            .collect()
    }

    fn height_for_width(&mut self, node: &Node, width: f32) -> f32 {
        match &node.kind {
            NodeKind::Text { text, style } => self.text_layout_size(text, style, width).height,
            NodeKind::Image(_) | NodeKind::Custom(_) => self.intrinsic_size(node).preferred.height,
            NodeKind::Container => {
                let children: Vec<_> = node
                    .children
                    .iter()
                    .filter(|child| child.floating.is_none())
                    .collect();
                let intrinsic: Vec<_> = children
                    .iter()
                    .map(|child| self.intrinsic_size(child))
                    .collect();
                let content_width = (width - node.layout.padding.horizontal()).max(0.0);
                let gap = node.layout.gap * children.len().saturating_sub(1) as f32;
                let width_available = (content_width - gap).max(0.0);
                let mut widths: Vec<_> = children
                    .iter()
                    .zip(&intrinsic)
                    .map(|(child, size)| match node.layout.direction {
                        Direction::Row => initial_axis(
                            child.layout.sizing.width,
                            size.preferred.width,
                            width_available,
                        ),
                        Direction::Column => cross_axis(
                            child.layout.sizing.width,
                            size.preferred.width,
                            size.minimum.width,
                            content_width,
                        ),
                    })
                    .collect();
                if node.layout.direction == Direction::Row {
                    let minimums: Vec<_> = children
                        .iter()
                        .zip(&intrinsic)
                        .map(|(child, size)| {
                            minimum_axis(child.layout.sizing.width, size.minimum.width)
                        })
                        .collect();
                    distribute_axis(
                        &children,
                        &mut widths,
                        &minimums,
                        width_available,
                        true,
                        !node.clip_x && !node.scroll_x,
                    );
                }
                let heights: Vec<_> = children
                    .iter()
                    .zip(widths)
                    .map(|(child, width)| {
                        intrinsic_axis(
                            child.layout.sizing.height,
                            self.height_for_width(child, width),
                        )
                    })
                    .collect();
                let children_height = match node.layout.direction {
                    Direction::Row => heights.into_iter().fold(0.0, f32::max),
                    Direction::Column => heights.into_iter().sum::<f32>() + gap,
                };
                intrinsic_axis(
                    node.layout.sizing.height,
                    children_height + node.layout.padding.vertical(),
                )
            }
        }
    }

    fn text_render_lines(
        &mut self,
        text: &str,
        style: &TextStyle,
        bounds: Rect,
    ) -> Vec<(String, Rect)> {
        let lines = self.wrap_text(text, style, bounds.width);
        let line_height = resolved_line_height(style);
        lines
            .into_iter()
            .enumerate()
            .map(|(index, line)| {
                let width = self.measure_text_cached(&line, style).width;
                let x = match style.align {
                    TextAlign::Left => bounds.x,
                    TextAlign::Center => bounds.x + (bounds.width - width).max(0.0) / 2.0,
                    TextAlign::Right => bounds.x + (bounds.width - width).max(0.0),
                };
                (
                    line,
                    Rect::new(x, bounds.y + index as f32 * line_height, width, line_height),
                )
            })
            .collect()
    }

    fn text_layout_size(&mut self, text: &str, style: &TextStyle, max_width: f32) -> Size {
        let lines = self.wrap_text(text, style, max_width);
        let width = lines
            .iter()
            .map(|line| self.measure_text_cached(line, style).width)
            .fold(0.0, f32::max)
            .min(max_width);
        Size::new(width, lines.len() as f32 * resolved_line_height(style))
    }

    fn wrap_text(&mut self, text: &str, style: &TextStyle, max_width: f32) -> Vec<String> {
        if text.is_empty() {
            return vec![String::new()];
        }

        match style.wrap {
            TextWrap::None => vec![text.replace('\n', " ")],
            TextWrap::Newlines => text.split('\n').map(str::to_owned).collect(),
            TextWrap::Words => {
                if !max_width.is_finite() {
                    return text.split('\n').map(str::to_owned).collect();
                }

                let mut lines = Vec::new();
                for paragraph in text.split('\n') {
                    let mut line = String::new();
                    for word in paragraph.split_whitespace() {
                        let candidate = if line.is_empty() {
                            word.to_string()
                        } else {
                            format!("{line} {word}")
                        };
                        if !line.is_empty()
                            && self.measure_text_cached(&candidate, style).width > max_width
                        {
                            lines.push(line);
                            line = word.to_string();
                        } else {
                            line = candidate;
                        }
                    }
                    lines.push(line);
                }
                lines
            }
        }
    }

    fn measure_text_cached(&mut self, text: &str, style: &TextStyle) -> Size {
        self.measure_text(text, style)
    }

    /// Measures text using the configured callback and cache.
    pub fn measure_text(&mut self, text: &str, style: &TextStyle) -> Size {
        let key = TextMeasureKey {
            text: text.to_string(),
            font_id: style.font_id,
            font_size: style.font_size.to_bits(),
            line_height: style.line_height.to_bits(),
            letter_spacing: style.letter_spacing.to_bits(),
        };
        if let Some(size) = self.measure_cache.get(&key) {
            return *size;
        }
        let size = (self.measure_text)(text, style);
        if self
            .max_measure_cache
            .is_some_and(|max_cache| self.measure_cache.len() >= max_cache)
        {
            self.measure_cache_exceeded = true;
            return size;
        }
        self.measure_cache.insert(key, size);
        size
    }

    /// Returns the nearest character index for an x coordinate in a single line.
    pub fn text_cursor_index_at_x(&mut self, text: &str, style: &TextStyle, x: f32) -> usize {
        let mut best = 0;
        let mut best_distance = x.abs();
        let char_count = text.chars().count();

        for index in 0..=char_count {
            let byte = char_index_to_byte(text, index);
            let width = self.measure_text(&text[..byte], style).width;
            let distance = (width - x).abs();
            if distance < best_distance {
                best = index;
                best_distance = distance;
            }
        }

        best
    }

    /// Creates a text selection from drag start and end x coordinates.
    pub fn text_selection_from_drag(
        &mut self,
        text: &str,
        style: &TextStyle,
        anchor_x: f32,
        focus_x: f32,
    ) -> TextSelection {
        TextSelection::new(
            self.text_cursor_index_at_x(text, style, anchor_x),
            self.text_cursor_index_at_x(text, style, focus_x),
        )
    }

    /// Returns x positions for selection handles.
    pub fn text_selection_handles(
        &mut self,
        text: &str,
        style: &TextStyle,
        selection: TextSelection,
    ) -> Option<(f32, f32)> {
        let (start, end) = selection.normalized()?;
        Some((
            self.measure_text(&text[..char_index_to_byte(text, start)], style)
                .width,
            self.measure_text(&text[..char_index_to_byte(text, end)], style)
                .width,
        ))
    }
}

#[allow(clippy::too_many_lines)]
fn distribute_axis(
    children: &[&Node],
    sizes: &mut [f32],
    minimums: &[f32],
    available: f32,
    x_axis: bool,
    compress: bool,
) {
    let rules: Vec<_> = children
        .iter()
        .map(|child| {
            if x_axis {
                child.layout.sizing.width
            } else {
                child.layout.sizing.height
            }
        })
        .collect();
    let resizable: Vec<_> = children
        .iter()
        .zip(&rules)
        .map(|(child, rule)| {
            matches!(rule, AxisSize::Fit { .. } | AxisSize::Grow { .. })
                && match &child.kind {
                    NodeKind::Text { style, .. } => style.wrap == TextWrap::Words,
                    _ => true,
                }
        })
        .collect();
    let mut overflow = if compress {
        (sizes.iter().sum::<f32>() - available.max(0.0)).max(0.0)
    } else {
        0.0
    };

    while overflow > 0.01 {
        let largest = resizable
            .iter()
            .zip(sizes.iter())
            .zip(minimums)
            .filter(|((resizable, size), min)| **resizable && **size > **min + 0.01)
            .map(|((_, size), _)| *size)
            .fold(0.0, f32::max);
        if largest == 0.0 {
            break;
        }
        let second = resizable
            .iter()
            .zip(sizes.iter())
            .zip(minimums)
            .filter(|((resizable, size), min)| {
                **resizable && **size > **min + 0.01 && **size < largest - 0.01
            })
            .map(|((_, size), _)| *size)
            .fold(0.0, f32::max);
        let count = resizable
            .iter()
            .zip(sizes.iter())
            .filter(|(resizable, size)| **resizable && (**size - largest).abs() < 0.01)
            .count() as f32;
        let step = (overflow / count).min(largest - second);
        let mut removed = 0.0;

        for ((resizable, size), min) in resizable.iter().zip(sizes.iter_mut()).zip(minimums) {
            if *resizable && (*size - largest).abs() < 0.01 {
                let next = (*size - step).max(*min);
                removed += *size - next;
                *size = next;
            }
        }
        if removed == 0.0 {
            break;
        }
        overflow -= removed;
    }

    let mut remaining = (available - sizes.iter().sum::<f32>()).max(0.0);
    let mut grow: Vec<_> = rules
        .iter()
        .enumerate()
        .filter_map(|(index, rule)| rule.is_grow().then_some(index))
        .collect();

    while remaining > 0.01 && !grow.is_empty() {
        let smallest = grow
            .iter()
            .map(|index| sizes[*index])
            .fold(f32::MAX, f32::min);
        let second = grow
            .iter()
            .map(|index| sizes[*index])
            .filter(|size| *size > smallest + 0.01)
            .fold(f32::MAX, f32::min);
        let count = grow
            .iter()
            .filter(|index| (sizes[**index] - smallest).abs() < 0.01)
            .count() as f32;
        let step = (remaining / count).min(second - smallest);
        let mut added = 0.0;

        grow.retain(|index| {
            if (sizes[*index] - smallest).abs() >= 0.01 {
                return true;
            }
            let AxisSize::Grow { max, .. } = rules[*index] else {
                return false;
            };
            let next = (sizes[*index] + step).min(max);
            added += next - sizes[*index];
            sizes[*index] = next;
            sizes[*index] < max - 0.01
        });
        if added == 0.0 {
            break;
        }
        remaining -= added;
    }
}

fn intrinsic_axis(rule: AxisSize, preferred: f32) -> f32 {
    match rule {
        AxisSize::Fit { min, max } | AxisSize::Grow { min, max } => preferred.clamp(min, max),
        AxisSize::Percent(_) => 0.0,
        AxisSize::Fixed(value) => value.max(0.0),
    }
}

fn minimum_axis(rule: AxisSize, minimum: f32) -> f32 {
    match rule {
        AxisSize::Fit { min, max } | AxisSize::Grow { min, max } => minimum.clamp(min, max),
        AxisSize::Percent(_) => 0.0,
        AxisSize::Fixed(value) => value.max(0.0),
    }
}

fn initial_axis(rule: AxisSize, preferred: f32, available: f32) -> f32 {
    match rule {
        AxisSize::Percent(percent) => available * percent.clamp(0.0, 1.0),
        _ => intrinsic_axis(rule, preferred),
    }
}

fn cross_axis(rule: AxisSize, preferred: f32, minimum: f32, available: f32) -> f32 {
    match rule {
        AxisSize::Grow { min, max } => available.clamp(min, max),
        AxisSize::Percent(percent) => available * percent.clamp(0.0, 1.0),
        AxisSize::Fit { min, max } => preferred
            .clamp(min, max)
            .min(available)
            .max(minimum.clamp(min, max)),
        AxisSize::Fixed(value) => value.max(0.0),
    }
}
