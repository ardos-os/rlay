use crate::frame::Frame;
use crate::geometry::{Color, Point, Radius, Rect, Size, Vector};
use crate::input::{InputState, PointerHit};
use crate::node::{Node, NodeKind};
use crate::result::{
    ClipRegion, CommandKind, ElementData, HitEntry, ImageRenderData, LayoutResult, RenderCommand,
    ScrollData,
};
use crate::scroll::ScrollState;
use crate::style::{
    AlignX, AlignY, Anchor, AttachTo, AxisSize, Direction, Floating, Layout, PointerCapture,
    Sizing, TextAlign, TextStyle, TextWrap,
};
use crate::text::{TextSelection, char_index_to_byte, main_axis, resolved_line_height};
use crate::transition::{
    Transition, TransitionArgs, TransitionExitOrdering, TransitionExitTrigger,
    TransitionInteraction, TransitionProperties, TransitionState, TransitionValues,
};
use std::collections::{HashMap, HashSet};

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
    transition_runtime: HashMap<u32, TransitionRuntime>,
    transition_bounds: HashMap<String, Rect>,
    transition_non_interactive: HashSet<String>,
    transition_exiting: HashSet<String>,
    previous_tree_ids: HashSet<String>,
    previous_viewport: Option<Size>,
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

#[derive(Clone)]
struct TransitionRuntime {
    config: Transition,
    state: TransitionState,
    initial: TransitionValues,
    current: TransitionValues,
    target: TransitionValues,
    elapsed: f32,
    active: TransitionProperties,
    parent: String,
    sibling_index: usize,
    relative: Point,
    snapshot: Node,
    exit_complete: bool,
    reparented: bool,
}

#[derive(Clone)]
struct TransitionNode {
    hash: u32,
    key: String,
    parent: String,
    sibling_index: usize,
    config: Transition,
    snapshot: Node,
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
    /// A node with a transition did not have a stable public id.
    TransitionMissingId,
    /// More than one node used the same stable id hash.
    DuplicateElementId(u32),
    /// Text nodes cannot use element transitions.
    TextTransitionUnsupported,
    /// A public id used an internal `__rlay_` prefix.
    ReservedElementId(String),
}

impl Engine {
    /// Creates an engine with a text measurement callback.
    pub fn new(measure_text: impl Fn(&str, &TextStyle) -> Size + 'static) -> Self {
        Self {
            measure_text: Box::new(measure_text),
            measure_cache: HashMap::new(),
            input: InputState::default(),
            scroll: ScrollState::default(),
            transition_runtime: HashMap::new(),
            transition_bounds: HashMap::new(),
            transition_non_interactive: HashSet::new(),
            transition_exiting: HashSet::new(),
            previous_tree_ids: HashSet::new(),
            previous_viewport: None,
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
    ///
    /// `delta_time` is elapsed frame time in seconds. Negative and non-finite
    /// values are treated as zero.
    #[allow(clippy::too_many_lines)]
    pub fn layout(&mut self, root: &Node, size: Size, delta_time: f32) -> LayoutResult {
        let delta_time = if delta_time.is_finite() && delta_time > 0.0 {
            delta_time
        } else {
            0.0
        };
        self.transition_runtime
            .retain(|_, runtime| !runtime.exit_complete);
        self.transition_bounds.clear();
        self.transition_non_interactive.clear();
        self.transition_exiting.clear();

        let mut errors = Vec::new();
        let mut tree = root.clone();
        let mut transition_nodes = Vec::new();
        let mut current_hashes = HashSet::new();
        normalize_tree(
            &mut tree,
            "__rlay_root",
            &mut transition_nodes,
            &mut current_hashes,
            &mut errors,
        );
        let current_transition_hashes: HashSet<_> =
            transition_nodes.iter().map(|node| node.hash).collect();
        let mut current_tree_ids = HashSet::new();
        collect_node_ids(&tree, &mut current_tree_ids);
        self.transition_runtime.retain(|hash, runtime| {
            current_transition_hashes.contains(hash)
                || (!current_hashes.contains(hash)
                    && (runtime.state == TransitionState::Exiting
                        || (runtime.config.has_exit()
                            && (runtime.config.exit.trigger
                                == TransitionExitTrigger::WhenParentExits
                                || current_tree_ids.contains(&runtime.parent)))))
        });

        let mut disappearing: Vec<_> = self
            .transition_runtime
            .iter()
            .filter(|(hash, runtime)| {
                !current_hashes.contains(hash)
                    && runtime.config.has_exit()
                    && (runtime.config.exit.trigger == TransitionExitTrigger::WhenParentExits
                        || current_tree_ids.contains(&runtime.parent))
            })
            .map(|(hash, _)| *hash)
            .collect();
        let nested: Vec<_> = disappearing
            .iter()
            .copied()
            .filter(|hash| {
                disappearing.iter().any(|ancestor| {
                    ancestor != hash
                        && self
                            .transition_runtime
                            .get(ancestor)
                            .is_some_and(|runtime| node_contains_hash(&runtime.snapshot, *hash))
                })
            })
            .collect();
        disappearing.retain(|hash| !nested.contains(hash));
        for hash in nested {
            self.transition_runtime.remove(&hash);
        }
        for hash in disappearing {
            if let Some(runtime) = self.transition_runtime.get_mut(&hash) {
                if runtime.state != TransitionState::Exiting {
                    runtime.state = TransitionState::Exiting;
                    runtime.initial = runtime.current;
                    runtime.target = runtime
                        .config
                        .exit_values(runtime.current, runtime.config.properties);
                    runtime.active = runtime.config.properties;
                    runtime.elapsed = 0.0;
                }
                let mut snapshot = runtime.snapshot.clone();
                remove_present_descendants(&mut snapshot, &current_hashes);
                snapshot.layout.sizing =
                    Sizing::fixed(runtime.current.bounds.width, runtime.current.bounds.height);
                if let Some(id) = &snapshot.id {
                    self.transition_exiting.insert(id.clone());
                }
                insert_exit(&mut tree, snapshot, runtime);
                collect_node_ids(&runtime.snapshot, &mut self.transition_non_interactive);
            }
        }

        let target_result = self.layout_pass(&tree, size);
        let viewport_changed = self.previous_viewport.is_some_and(|old| old != size);
        let appeared_ids: HashSet<_> = target_result
            .elements
            .keys()
            .filter(|id| !self.previous_tree_ids.contains(*id))
            .cloned()
            .collect();

        for info in &transition_nodes {
            let Some(element) = target_result.elements.get(&info.key) else {
                continue;
            };
            let Some(node) = find_node(&tree, &info.key) else {
                continue;
            };
            let target = values(node, element.bounds);
            let parent_bounds = target_result
                .elements
                .get(&info.parent)
                .map_or(Rect::default(), |parent| parent.bounds);
            let parent_scroll = target_result
                .scroll_containers
                .get(&info.parent)
                .map_or(Vector::ZERO, |scroll| scroll.offset);
            let relative = Point::new(
                target.bounds.x - parent_bounds.x + parent_scroll.x,
                target.bounds.y - parent_bounds.y + parent_scroll.y,
            );
            let parent_appeared = appeared_ids.contains(&info.parent);

            if let Some(runtime) = self.transition_runtime.get_mut(&info.hash) {
                let reparented = runtime.parent != info.parent;
                let was_exiting = runtime.state == TransitionState::Exiting;
                let changed = changed_properties(
                    runtime.target,
                    target,
                    info.config.properties,
                    runtime.relative,
                    relative,
                    reparented,
                    viewport_changed,
                );
                runtime.config = info.config;
                runtime.parent.clone_from(&info.parent);
                runtime.reparented = reparented;
                runtime.sibling_index = info.sibling_index;
                runtime.relative = relative;
                runtime.snapshot = info.snapshot.clone();
                sync_unselected(&mut runtime.current, target, info.config.properties);
                runtime.target = target;
                if was_exiting || !changed.is_empty() {
                    runtime.state = TransitionState::Transitioning;
                    runtime.initial = runtime.current;
                    runtime.active = if was_exiting {
                        info.config.properties
                    } else {
                        runtime.active | changed
                    };
                    runtime.elapsed = 0.0;
                    runtime.exit_complete = false;
                }
            } else {
                let should_enter = info.config.has_enter()
                    && (!parent_appeared
                        || info.config.enter.trigger
                            == crate::transition::TransitionEnterTrigger::OnFirstParentFrame);
                let current = if should_enter {
                    info.config.enter_values(target, info.config.properties)
                } else {
                    target
                };
                self.transition_runtime.insert(
                    info.hash,
                    TransitionRuntime {
                        config: info.config,
                        state: if should_enter {
                            TransitionState::Entering
                        } else {
                            TransitionState::Idle
                        },
                        initial: current,
                        current,
                        target,
                        elapsed: 0.0,
                        active: if should_enter {
                            info.config.properties
                        } else {
                            TransitionProperties::empty()
                        },
                        parent: info.parent.clone(),
                        sibling_index: info.sibling_index,
                        relative,
                        snapshot: info.snapshot.clone(),
                        exit_complete: false,
                        reparented: false,
                    },
                );
            }
        }

        for runtime in self.transition_runtime.values_mut() {
            if runtime.state != TransitionState::Idle
                && !(runtime.state == TransitionState::Entering && runtime.elapsed == 0.0)
            {
                let frame = runtime.config.frame(TransitionArgs {
                    state: runtime.state,
                    initial: runtime.initial,
                    current: runtime.current,
                    target: runtime.target,
                    elapsed: runtime.elapsed,
                    duration: runtime.config.duration,
                    properties: runtime.active,
                });
                runtime.current = frame.values;
                if frame.complete {
                    if runtime.state == TransitionState::Exiting {
                        runtime.exit_complete = true;
                    } else {
                        runtime.state = TransitionState::Idle;
                        runtime.current = runtime.target;
                        runtime.active = TransitionProperties::empty();
                    }
                }
            }
            if runtime.state != TransitionState::Idle {
                runtime.elapsed += delta_time;
            }
        }

        apply_transition_values(
            &mut tree,
            &self.transition_runtime,
            &mut self.transition_bounds,
            &mut self.transition_non_interactive,
        );
        let mut result = self.layout_pass(&tree, size);
        strip_internal_ids(&mut result);
        result.errors.extend(errors);
        self.finish_layout(&mut result, size);
        self.previous_tree_ids = target_result.elements.keys().cloned().collect();
        self.previous_viewport = Some(size);
        result
    }

    fn layout_pass(&mut self, root: &Node, size: Size) -> LayoutResult {
        let mut result = LayoutResult::default();
        let root_bounds = Rect::new(0.0, 0.0, size.width, size.height);
        self.layout_node(root, root_bounds, root_bounds, &mut result);
        result
    }

    fn finish_layout(&mut self, result: &mut LayoutResult, size: Size) {
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
        let root_bounds = Rect::new(0.0, 0.0, size.width, size.height);
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
                    .or_else(|| Engine::hit_test(result, pointer.position).map(str::to_owned)),
                mouse_button: pointer.mouse_button,
                gesture: pointer.gesture,
            })
            .collect();
        self.scroll.finish_layout_frame(&mut self.input, result);
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
        let bounds = node
            .id
            .as_deref()
            .and_then(|id| self.transition_bounds.get(id).copied())
            .unwrap_or(bounds);
        let current_clip = parent_clip.add(bounds, clip_x, clip_y);
        let hit_bounds = parent_clip.apply(bounds);
        let culled = self.culling && bounds.intersection(viewport).is_none();

        let hit_testable = !node
            .id
            .as_ref()
            .is_some_and(|id| self.transition_non_interactive.contains(id))
            && node
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
            NodeKind::Image(image) => {
                if !culled {
                    result.commands.push(RenderCommand {
                        id: node.id.clone(),
                        bounds,
                        kind: CommandKind::Image(ImageRenderData {
                            background_color: node.background,
                            corner_radius: node.border.radius,
                            image_id: image.image_id,
                        }),
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
        let flow_children: Vec<_> = normal_children
            .iter()
            .copied()
            .filter(|child| {
                child
                    .id
                    .as_ref()
                    .is_none_or(|id| !self.transition_exiting.contains(id))
            })
            .collect();
        let mut floating_children: Vec<_> = node
            .children
            .iter()
            .filter(|child| child.floating.is_some())
            .collect();
        floating_children.sort_by_key(|child| child.floating.as_ref().map_or(0, |f| f.z_index));

        let child_sizes = self.resolve_children_sizes(
            &flow_children,
            Size::new(content.width, content.height),
            node,
        );
        let used_main = node.layout.gap * flow_children.len().saturating_sub(1) as f32
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

        let mut child_sizes = flow_children.into_iter().zip(child_sizes);
        for child in normal_children {
            let exiting = child
                .id
                .as_ref()
                .is_some_and(|id| self.transition_exiting.contains(id));
            let child_size = if exiting {
                let fit = self.measure_node(child);
                self.resolve_child_size(
                    child,
                    fit,
                    content.width,
                    content.height,
                    None,
                    node.layout.direction,
                )
            } else {
                child_sizes.next().expect("flow child size").1
            };
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
            if !exiting {
                cursor += main_axis(child_size, node.layout.direction) + node.layout.gap;
            }
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
            NodeKind::Image(_) | NodeKind::Custom(_) => {
                let mut preferred = Size::new(
                    intrinsic_axis(node.layout.sizing.width, 0.0),
                    intrinsic_axis(node.layout.sizing.height, 0.0),
                );
                update_aspect_ratio_size(&mut preferred, node.aspect_ratio);
                IntrinsicSize {
                    preferred,
                    minimum: preferred,
                }
            }
            NodeKind::Container => {
                let mut preferred_main: f32 = 0.0;
                let mut preferred_cross: f32 = 0.0;
                let mut minimum_main: f32 = 0.0;
                let mut minimum_cross: f32 = 0.0;
                let children: Vec<_> = node
                    .children
                    .iter()
                    .filter(|child| {
                        child.floating.is_none()
                            && child
                                .id
                                .as_ref()
                                .is_none_or(|id| !self.transition_exiting.contains(id))
                    })
                    .collect();
                for (index, child) in children.into_iter().enumerate() {
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

    #[allow(clippy::too_many_lines)]
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
            .map(|(child, width)| {
                child
                    .aspect_ratio
                    .filter(|ratio| *ratio != 0.0 && *width != 0.0)
                    .map_or_else(
                        || self.height_for_width(child, *width),
                        |ratio| *width / ratio,
                    )
            })
            .collect();
        let height_available = (available.height - gap).max(0.0);
        let mut heights: Vec<_> = children
            .iter()
            .zip(&intrinsic)
            .zip(&natural_heights)
            .map(|((child, size), natural)| {
                if child.aspect_ratio.is_some() && *natural != 0.0 {
                    *natural
                } else {
                    match parent.layout.direction {
                        Direction::Row => cross_axis(
                            child.layout.sizing.height,
                            *natural,
                            size.minimum.height,
                            available.height,
                        ),
                        Direction::Column => {
                            initial_axis(child.layout.sizing.height, *natural, height_available)
                        }
                    }
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
            .map(|((child, mut width), height)| {
                if let Some(ratio) = child.aspect_ratio
                    && ratio != 0.0
                {
                    width = ratio * height;
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
                    .filter(|child| {
                        child.floating.is_none()
                            && child
                                .id
                                .as_ref()
                                .is_none_or(|id| !self.transition_exiting.contains(id))
                    })
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

fn normalize_tree(
    node: &mut Node,
    path: &str,
    transitions: &mut Vec<TransitionNode>,
    hashes: &mut HashSet<u32>,
    errors: &mut Vec<LayoutError>,
) {
    if !normalize_node(node, path, "", 0, transitions, hashes, errors) {
        *node = Node::new();
    }
}

#[allow(clippy::too_many_arguments)]
fn normalize_node(
    node: &mut Node,
    path: &str,
    parent: &str,
    sibling_index: usize,
    transitions: &mut Vec<TransitionNode>,
    hashes: &mut HashSet<u32>,
    errors: &mut Vec<LayoutError>,
) -> bool {
    if let Some(id) = &node.id
        && id.starts_with("__rlay_")
    {
        errors.push(LayoutError::ReservedElementId(id.clone()));
        return false;
    }
    let stable = node.element_id.clone();
    if let Some(element_id) = &stable
        && !hashes.insert(element_id.hash)
    {
        errors.push(LayoutError::DuplicateElementId(element_id.hash));
    }
    if node.id.is_none() {
        node.id = Some(path.to_owned());
    }
    let key = node.id.clone().expect("assigned internal id");

    if node.transition.is_some() && stable.is_none() {
        errors.push(LayoutError::TransitionMissingId);
    }
    if node.transition.is_some() && matches!(node.kind, NodeKind::Text { .. }) {
        errors.push(LayoutError::TextTransitionUnsupported);
        node.transition = None;
    }

    let mut index = 0;
    node.children.retain_mut(|child| {
        let keep = normalize_node(
            child,
            &format!("{path}/{index}"),
            &key,
            index,
            transitions,
            hashes,
            errors,
        );
        index += usize::from(keep);
        keep
    });

    if let (Some(config), Some(element_id)) = (node.transition, stable) {
        transitions.push(TransitionNode {
            hash: element_id.hash,
            key,
            parent: parent.to_owned(),
            sibling_index,
            config,
            snapshot: node.clone(),
        });
    }
    true
}

fn values(node: &Node, bounds: Rect) -> TransitionValues {
    TransitionValues {
        bounds,
        background: node.background,
        overlay: node.overlay.unwrap_or(Color::TRANSPARENT),
        radius: node.border.radius,
        border_color: node.border.color,
        border_width: node.border.width,
    }
}

fn changed_properties(
    old: TransitionValues,
    new: TransitionValues,
    configured: TransitionProperties,
    old_relative: Point,
    new_relative: Point,
    reparented: bool,
    viewport_changed: bool,
) -> TransitionProperties {
    let mut changed = TransitionProperties::empty();
    if configured.contains(TransitionProperties::X)
        && float_changed(old.bounds.x, new.bounds.x)
        && (reparented || float_changed(old_relative.x, new_relative.x))
        && !viewport_changed
    {
        changed |= TransitionProperties::X;
    }
    if configured.contains(TransitionProperties::Y)
        && float_changed(old.bounds.y, new.bounds.y)
        && (reparented || float_changed(old_relative.y, new_relative.y))
        && !viewport_changed
    {
        changed |= TransitionProperties::Y;
    }
    if configured.contains(TransitionProperties::WIDTH)
        && float_changed(old.bounds.width, new.bounds.width)
        && !viewport_changed
    {
        changed |= TransitionProperties::WIDTH;
    }
    if configured.contains(TransitionProperties::HEIGHT)
        && float_changed(old.bounds.height, new.bounds.height)
        && !viewport_changed
    {
        changed |= TransitionProperties::HEIGHT;
    }
    if configured.contains(TransitionProperties::BACKGROUND_COLOR)
        && old.background != new.background
    {
        changed |= TransitionProperties::BACKGROUND_COLOR;
    }
    if configured.contains(TransitionProperties::OVERLAY_COLOR) && old.overlay != new.overlay {
        changed |= TransitionProperties::OVERLAY_COLOR;
    }
    if configured.contains(TransitionProperties::CORNER_RADIUS) && old.radius != new.radius {
        changed |= TransitionProperties::CORNER_RADIUS;
    }
    if configured.contains(TransitionProperties::BORDER_COLOR)
        && old.border_color != new.border_color
    {
        changed |= TransitionProperties::BORDER_COLOR;
    }
    if configured.contains(TransitionProperties::BORDER_WIDTH)
        && old.border_width != new.border_width
    {
        changed |= TransitionProperties::BORDER_WIDTH;
    }
    changed
}

fn float_changed(left: f32, right: f32) -> bool {
    (left - right).abs() >= 0.01
}

fn sync_unselected(
    current: &mut TransitionValues,
    target: TransitionValues,
    selected: TransitionProperties,
) {
    if !selected.contains(TransitionProperties::X) {
        current.bounds.x = target.bounds.x;
    }
    if !selected.contains(TransitionProperties::Y) {
        current.bounds.y = target.bounds.y;
    }
    if !selected.contains(TransitionProperties::WIDTH) {
        current.bounds.width = target.bounds.width;
    }
    if !selected.contains(TransitionProperties::HEIGHT) {
        current.bounds.height = target.bounds.height;
    }
    if !selected.contains(TransitionProperties::BACKGROUND_COLOR) {
        current.background = target.background;
    }
    if !selected.contains(TransitionProperties::OVERLAY_COLOR) {
        current.overlay = target.overlay;
    }
    if !selected.contains(TransitionProperties::CORNER_RADIUS) {
        current.radius = target.radius;
    }
    if !selected.contains(TransitionProperties::BORDER_COLOR) {
        current.border_color = target.border_color;
    }
    if !selected.contains(TransitionProperties::BORDER_WIDTH) {
        current.border_width = target.border_width;
    }
}

fn apply_transition_values(
    node: &mut Node,
    runtimes: &HashMap<u32, TransitionRuntime>,
    bounds: &mut HashMap<String, Rect>,
    non_interactive: &mut HashSet<String>,
) {
    if let Some(element_id) = &node.element_id
        && let Some(runtime) = runtimes.get(&element_id.hash)
        && runtime.state != TransitionState::Idle
    {
        let p = runtime.active;
        let current = runtime.current;
        if p.contains(TransitionProperties::WIDTH) && !runtime.reparented {
            node.layout.sizing.width = AxisSize::fixed(current.bounds.width);
        }
        if p.contains(TransitionProperties::HEIGHT) && !runtime.reparented {
            node.layout.sizing.height = AxisSize::fixed(current.bounds.height);
        }
        if (runtime.state == TransitionState::Exiting || p.intersects(TransitionProperties::BOUNDS))
            && let Some(id) = &node.id
        {
            bounds.insert(id.clone(), current.bounds);
        }
        if p.contains(TransitionProperties::BACKGROUND_COLOR) {
            node.background = current.background;
        }
        if p.contains(TransitionProperties::OVERLAY_COLOR) {
            node.overlay = Some(current.overlay);
        }
        if p.contains(TransitionProperties::CORNER_RADIUS) {
            node.border.radius = current.radius;
        }
        if p.contains(TransitionProperties::BORDER_COLOR) {
            node.border.color = current.border_color;
        }
        if p.contains(TransitionProperties::BORDER_WIDTH) {
            node.border.width = current.border_width;
        }
        if p.intersects(TransitionProperties::POSITION)
            && runtime.config.interaction == TransitionInteraction::Disable
            && let Some(id) = &node.id
        {
            non_interactive.insert(id.clone());
        }
    }
    for child in &mut node.children {
        apply_transition_values(child, runtimes, bounds, non_interactive);
    }
}

fn find_node<'a>(node: &'a Node, id: &str) -> Option<&'a Node> {
    if node.id.as_deref() == Some(id) {
        return Some(node);
    }
    node.children.iter().find_map(|child| find_node(child, id))
}

fn find_node_mut<'a>(node: &'a mut Node, id: &str) -> Option<&'a mut Node> {
    if node.id.as_deref() == Some(id) {
        return Some(node);
    }
    node.children
        .iter_mut()
        .find_map(|child| find_node_mut(child, id))
}

fn insert_exit(tree: &mut Node, mut snapshot: Node, runtime: &TransitionRuntime) {
    if let Some(parent) = find_node_mut(tree, &runtime.parent) {
        let index = match runtime.config.exit.sibling_ordering {
            TransitionExitOrdering::Underneath => 0,
            TransitionExitOrdering::Natural => runtime.sibling_index.min(parent.children.len()),
            TransitionExitOrdering::Above => parent.children.len(),
        };
        parent.children.insert(index, snapshot);
    } else {
        snapshot.floating = Some(Floating {
            attach_to: AttachTo::Root,
            element_anchor: Anchor::TOP_LEFT,
            target_anchor: Anchor::TOP_LEFT,
            offset: Vector::new(runtime.current.bounds.x, runtime.current.bounds.y),
            z_index: 0,
            pointer_capture: PointerCapture::PassThrough,
            clip_to_parent: false,
        });
        tree.children.push(snapshot);
    }
}

fn remove_present_descendants(node: &mut Node, present: &HashSet<u32>) {
    node.children.retain(|child| {
        child
            .element_id
            .as_ref()
            .is_none_or(|id| !present.contains(&id.hash))
    });
    for child in &mut node.children {
        remove_present_descendants(child, present);
    }
}

fn collect_node_ids(node: &Node, ids: &mut HashSet<String>) {
    if let Some(id) = &node.id {
        ids.insert(id.clone());
    }
    for child in &node.children {
        collect_node_ids(child, ids);
    }
}

fn node_contains_hash(node: &Node, hash: u32) -> bool {
    node.element_id.as_ref().is_some_and(|id| id.hash == hash)
        || node
            .children
            .iter()
            .any(|child| node_contains_hash(child, hash))
}

fn strip_internal_ids(result: &mut LayoutResult) {
    result.elements.retain(|id, _| !id.starts_with("__rlay_"));
    result
        .scroll_containers
        .retain(|id, _| !id.starts_with("__rlay_"));
    result
        .hit_order
        .retain(|hit| !hit.id.starts_with("__rlay_"));
    for command in &mut result.commands {
        if command
            .id
            .as_deref()
            .is_some_and(|id| id.starts_with("__rlay_"))
        {
            command.id = None;
        }
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

fn update_aspect_ratio_size(size: &mut Size, aspect_ratio: Option<f32>) {
    let Some(ratio) = aspect_ratio.filter(|ratio| *ratio != 0.0) else {
        return;
    };
    if size.width == 0.0 && size.height != 0.0 {
        size.width = size.height * ratio;
    } else if size.width != 0.0 && size.height == 0.0 {
        size.height = size.width / ratio;
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
