use bitflags::bitflags;

use crate::{Color, Padding, Radius, Rect};

bitflags! {
    /// Properties that may be animated by a transition.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct TransitionProperties: u16 {
        /// Horizontal position.
        const X = 1;
        /// Vertical position.
        const Y = 2;
        /// Both position axes.
        const POSITION = Self::X.bits() | Self::Y.bits();
        /// Width.
        const WIDTH = 4;
        /// Height.
        const HEIGHT = 8;
        /// Width and height.
        const DIMENSIONS = Self::WIDTH.bits() | Self::HEIGHT.bits();
        /// Position and dimensions.
        const BOUNDS = Self::POSITION.bits() | Self::DIMENSIONS.bits();
        /// Background color.
        const BACKGROUND_COLOR = 16;
        /// Overlay color.
        const OVERLAY_COLOR = 32;
        /// Corner radius.
        const CORNER_RADIUS = 64;
        /// Border color.
        const BORDER_COLOR = 128;
        /// Border widths.
        const BORDER_WIDTH = 256;
        /// Border color and widths.
        const BORDER = Self::BORDER_COLOR.bits() | Self::BORDER_WIDTH.bits();
    }
}

/// Current transition phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransitionState {
    /// No animation is active.
    #[default]
    Idle,
    /// The element is appearing.
    Entering,
    /// Existing values are changing.
    Transitioning,
    /// The element is disappearing.
    Exiting,
}

/// Geometry and paint values supplied to transition callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TransitionValues {
    /// Element bounds.
    pub bounds: Rect,
    /// Background color.
    pub background: Color,
    /// Overlay color, with `None` represented as transparent while animating.
    pub overlay: Color,
    /// Corner radii.
    pub radius: Radius,
    /// Border color.
    pub border_color: Color,
    /// Border widths.
    pub border_width: Padding,
}

/// Input passed to a transition handler.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionArgs {
    /// Current transition phase.
    pub state: TransitionState,
    /// Values at the start of this animation.
    pub initial: TransitionValues,
    /// Values rendered in the previous frame.
    pub current: TransitionValues,
    /// Current target values.
    pub target: TransitionValues,
    /// Elapsed animation time in seconds.
    pub elapsed: f32,
    /// Requested duration in seconds.
    pub duration: f32,
    /// Properties active in this animation.
    pub properties: TransitionProperties,
}

/// Output returned by a transition handler.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionFrame {
    /// Values to render this frame.
    pub values: TransitionValues,
    /// Whether this animation has reached its target.
    pub complete: bool,
}

/// Controls hit testing while position is animated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransitionInteraction {
    /// Ignore interactions while x or y is transitioning.
    #[default]
    Disable,
    /// Keep interactions enabled.
    Allow,
}

/// Controls enter behavior when the parent also appeared this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransitionEnterTrigger {
    /// Do not enter when the parent is new.
    #[default]
    SkipOnFirstParentFrame,
    /// Enter even when the parent is new.
    OnFirstParentFrame,
}

/// Controls exit behavior when the parent also disappears.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransitionExitTrigger {
    /// Remove the child with its parent.
    #[default]
    SkipWhenParentExits,
    /// Run the child exit independently.
    WhenParentExits,
}

/// Placement of an exiting subtree relative to current siblings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransitionExitOrdering {
    /// Place it before all current siblings.
    #[default]
    Underneath,
    /// Restore its previous sibling index.
    Natural,
    /// Place it after all current siblings.
    Above,
}

/// Enter transition configuration.
#[derive(Debug, Clone, Copy, Default)]
pub struct TransitionEnter {
    /// Maps the first target values to the values rendered on the first frame.
    pub initial: Option<fn(TransitionValues, TransitionProperties) -> TransitionValues>,
    /// Parent appearance behavior.
    pub trigger: TransitionEnterTrigger,
}

/// Exit transition configuration.
#[derive(Debug, Clone, Copy, Default)]
pub struct TransitionExit {
    /// Maps the last values to the exit target.
    pub target: Option<fn(TransitionValues, TransitionProperties) -> TransitionValues>,
    /// Parent disappearance behavior.
    pub trigger: TransitionExitTrigger,
    /// Placement relative to surviving siblings.
    pub sibling_ordering: TransitionExitOrdering,
}

/// Declarative transition configuration attached to a node.
#[derive(Debug, Clone, Copy)]
pub struct Transition {
    /// Pure animation callback.
    pub handler: fn(TransitionArgs) -> TransitionFrame,
    /// Duration in seconds.
    pub duration: f32,
    /// Properties eligible for animation.
    pub properties: TransitionProperties,
    /// Hit-testing behavior during position transitions.
    pub interaction: TransitionInteraction,
    /// Optional enter configuration.
    pub enter: TransitionEnter,
    /// Optional exit configuration.
    pub exit: TransitionExit,
}

impl Transition {
    /// Creates a transition using Clay's cubic ease-out curve.
    #[must_use]
    pub const fn ease_out(duration: f32, properties: TransitionProperties) -> Self {
        Self {
            handler: ease_out,
            duration,
            properties,
            interaction: TransitionInteraction::Disable,
            enter: TransitionEnter {
                initial: None,
                trigger: TransitionEnterTrigger::SkipOnFirstParentFrame,
            },
            exit: TransitionExit {
                target: None,
                trigger: TransitionExitTrigger::SkipWhenParentExits,
                sibling_ordering: TransitionExitOrdering::Underneath,
            },
        }
    }
}

/// Clay-compatible cubic ease-out transition handler.
#[must_use]
pub fn ease_out(args: TransitionArgs) -> TransitionFrame {
    let ratio = if args.duration > 0.0 {
        (args.elapsed / args.duration).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let amount = 1.0 - (1.0 - ratio).powi(3);
    let mut values = args.current;
    let p = args.properties;

    if p.contains(TransitionProperties::X) {
        values.bounds.x = lerp(args.initial.bounds.x, args.target.bounds.x, amount);
    }
    if p.contains(TransitionProperties::Y) {
        values.bounds.y = lerp(args.initial.bounds.y, args.target.bounds.y, amount);
    }
    if p.contains(TransitionProperties::WIDTH) {
        values.bounds.width = lerp(args.initial.bounds.width, args.target.bounds.width, amount);
    }
    if p.contains(TransitionProperties::HEIGHT) {
        values.bounds.height = lerp(
            args.initial.bounds.height,
            args.target.bounds.height,
            amount,
        );
    }
    if p.contains(TransitionProperties::BACKGROUND_COLOR) {
        values.background = lerp_color(args.initial.background, args.target.background, amount);
    }
    if p.contains(TransitionProperties::OVERLAY_COLOR) {
        values.overlay = lerp_color(args.initial.overlay, args.target.overlay, amount);
    }
    if p.contains(TransitionProperties::CORNER_RADIUS) {
        values.radius = Radius {
            top_left: lerp(
                args.initial.radius.top_left,
                args.target.radius.top_left,
                amount,
            ),
            top_right: lerp(
                args.initial.radius.top_right,
                args.target.radius.top_right,
                amount,
            ),
            bottom_left: lerp(
                args.initial.radius.bottom_left,
                args.target.radius.bottom_left,
                amount,
            ),
            bottom_right: lerp(
                args.initial.radius.bottom_right,
                args.target.radius.bottom_right,
                amount,
            ),
        };
    }
    if p.contains(TransitionProperties::BORDER_COLOR) {
        values.border_color =
            lerp_color(args.initial.border_color, args.target.border_color, amount);
    }
    if p.contains(TransitionProperties::BORDER_WIDTH) {
        values.border_width =
            lerp_padding(args.initial.border_width, args.target.border_width, amount);
    }

    TransitionFrame {
        values,
        complete: ratio >= 1.0,
    }
}

fn lerp(from: f32, to: f32, amount: f32) -> f32 {
    from + (to - from) * amount
}

fn lerp_color(from: Color, to: Color, amount: f32) -> Color {
    Color::rgba(
        lerp(from.r, to.r, amount),
        lerp(from.g, to.g, amount),
        lerp(from.b, to.b, amount),
        lerp(from.a, to.a, amount),
    )
}

fn lerp_padding(from: Padding, to: Padding, amount: f32) -> Padding {
    Padding::new(
        lerp(from.left, to.left, amount),
        lerp(from.right, to.right, amount),
        lerp(from.top, to.top, amount),
        lerp(from.bottom, to.bottom, amount),
    )
}
