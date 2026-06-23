use rlay::{
    AxisSize, CommandKind, Direction, Engine, Layout, LayoutError, Node, Point, Rect, Size, Sizing,
    TextSelection, TextStyle, Vector, ease_out,
};

fn engine() -> Engine {
    Engine::new(|text, style| {
        Size::new(
            text.chars().count() as f32 * style.font_size,
            style.font_size,
        )
    })
}

#[test]
fn frame_exposes_current_open_element_id() {
    let mut engine = engine();
    let mut frame = engine.begin(Size::new(10.0, 10.0));
    frame.open(Node::new().id("panel"));

    assert_eq!(frame.open_element_id().unwrap().label, "panel");
}

#[test]
fn external_scroll_query_overrides_internal_offset() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().id("row").layout(Layout {
            sizing: Sizing {
                width: AxisSize::fixed(20.0),
                height: AxisSize::fixed(40.0),
            },
            ..Layout::default()
        }));
    let mut engine = engine();
    engine.set_scroll_offset("list", Vector::new(0.0, 5.0));
    engine.set_query_scroll_offset(|id| {
        if id == "list" {
            Vector::new(0.0, 12.0)
        } else {
            Vector::ZERO
        }
    });

    let result = engine.layout(&root, Size::new(20.0, 20.0));

    assert_eq!(
        result.scroll_container("list").unwrap().offset,
        Vector::new(0.0, 12.0)
    );
    assert_eq!(
        result.element("row").unwrap().bounds,
        Rect::new(0.0, -12.0, 20.0, 40.0)
    );
}

#[test]
fn debug_mode_emits_debug_overlay_command() {
    let mut engine = engine();
    engine.set_debug(true);

    let result = engine.layout(&Node::new().id("root"), Size::new(10.0, 10.0));

    assert!(
        result
            .commands
            .iter()
            .any(|command| command.id.as_deref() == Some("__rlay_debug_panel"))
    );
    assert!(
        result
            .commands
            .iter()
            .any(|command| command.id.as_deref() == Some("__rlay_debug_text"))
    );
    assert!(matches!(
        result.commands.last().unwrap().kind,
        CommandKind::DebugOverlay { elements: 1, .. }
    ));
}

#[test]
fn ease_out_interpolates_towards_target() {
    assert_eq!(ease_out(0.0, 10.0, 0.0, 1.0), 0.0);
    assert_eq!(ease_out(0.0, 10.0, 1.0, 1.0), 10.0);
    assert!(ease_out(0.0, 10.0, 0.5, 1.0) > 5.0);
}

#[test]
fn text_selection_normalizes_drag_range() {
    let mut engine = engine();
    let style = TextStyle::default();
    let selection = engine.text_selection_from_drag("abcd", &style, 48.0, 16.0);

    assert_eq!(selection, TextSelection::new(3, 1));
    assert_eq!(selection.normalized(), Some((1, 3)));
}

#[test]
fn element_capacity_limit_reports_error_without_panicking() {
    let root = Node::new()
        .id("root")
        .child(Node::new().id("child").layout(Layout {
            sizing: Sizing::fixed(1.0, 1.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    engine.set_max_elements(Some(1));

    let result = engine.layout(&root, Size::new(10.0, 10.0));

    assert_eq!(result.errors, vec![LayoutError::ElementsCapacityExceeded]);
}

#[test]
fn frame_hovered_uses_previous_layout_result() {
    let root = Node::new().id("button").layout(Layout {
        sizing: Sizing::fixed(10.0, 10.0),
        ..Layout::default()
    });
    let mut engine = engine();
    engine.input_mut().set_mouse_position(Point::new(1.0, 1.0));
    let result = engine.layout(&root, Size::new(10.0, 10.0));

    let mut frame = engine.begin(Size::new(10.0, 10.0));
    frame.open(Node::new().id("button"));

    assert!(frame.hovered(&result));
}

#[test]
fn transition_bounds_override_layout_bounds() {
    let root = Node::new().id("panel").layout(Layout {
        sizing: Sizing::fixed(10.0, 10.0),
        ..Layout::default()
    });
    let mut engine = engine();
    engine.set_transition_bounds("panel", Rect::new(5.0, 6.0, 7.0, 8.0));

    let result = engine.layout(&root, Size::new(20.0, 20.0));

    assert_eq!(
        result.element("panel").unwrap().bounds,
        Rect::new(5.0, 6.0, 7.0, 8.0)
    );
}

#[test]
fn transition_bounds_interpolates_layout_bounds() {
    let root = Node::new().id("panel").layout(Layout {
        sizing: Sizing::fixed(10.0, 10.0),
        ..Layout::default()
    });
    let mut engine = engine();
    engine.transition_bounds(
        "panel",
        Rect::new(0.0, 0.0, 10.0, 10.0),
        Rect::new(10.0, 10.0, 20.0, 20.0),
        0.5,
        1.0,
    );

    let result = engine.layout(&root, Size::new(30.0, 30.0));

    assert_eq!(
        result.element("panel").unwrap().bounds,
        Rect::new(7.5, 7.5, 17.5, 17.5)
    );
}

#[test]
fn transition_exit_commands_are_retained_after_element_disappears() {
    let mut engine = engine();
    engine.transition_exit_commands(
        "toast",
        vec![rlay::RenderCommand {
            id: Some("toast".into()),
            bounds: Rect::new(1.0, 2.0, 3.0, 4.0),
            kind: CommandKind::Custom(99, rlay::Radius::default()),
        }],
    );

    let result = engine.layout(&Node::new().id("root"), Size::new(10.0, 10.0));

    assert!(result.commands.iter().any(|command| {
        command.id.as_deref() == Some("toast") && matches!(command.kind, CommandKind::Custom(99, _))
    }));
}

#[test]
fn additional_capacity_limits_report_errors() {
    let root = Node::new()
        .background(rlay::Color::rgba(1.0, 1.0, 1.0, 255.0))
        .child(Node::text("abc", TextStyle::default()).id("text"));
    let mut engine = engine();
    engine.set_max_commands(Some(0));
    engine.set_max_measure_text_cache_entries(Some(0));

    let result = engine.layout(&root, Size::new(100.0, 100.0));

    assert!(
        result
            .errors
            .contains(&LayoutError::CommandsCapacityExceeded)
    );
    assert!(
        result
            .errors
            .contains(&LayoutError::TextMeasurementCapacityExceeded)
    );
}

#[test]
fn text_selection_handles_return_prefix_positions() {
    let mut engine = engine();
    let style = TextStyle::default();

    assert_eq!(
        engine.text_selection_handles("abcd", &style, TextSelection::new(1, 3)),
        Some((16.0, 48.0))
    );
}
