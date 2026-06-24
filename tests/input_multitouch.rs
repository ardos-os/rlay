use rlay::{
    Direction, Engine, Layout, Node, Point, PointerHit, PointerId, PointerPhase, Size, Sizing,
    TextStyle,
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
fn layout_result_reports_independent_touch_targets() {
    let root = Node::new()
        .layout(Layout {
            direction: Direction::Row,
            ..Layout::default()
        })
        .child(Node::new().id("left").layout(Layout {
            sizing: Sizing::fixed(100.0, 100.0),
            ..Layout::default()
        }))
        .child(Node::new().id("right").layout(Layout {
            sizing: Sizing::fixed(100.0, 100.0),
            ..Layout::default()
        }));
    let mut engine = engine();

    engine
        .input_mut()
        .set_touch(10, Point::new(25.0, 25.0), true);
    engine
        .input_mut()
        .set_touch(20, Point::new(125.0, 25.0), true);
    let result = engine.layout(&root, Size::new(200.0, 100.0), 0.0);

    assert!(result.pointers.contains(&PointerHit {
        pointer_id: PointerId::Touch(10),
        position: Point::new(25.0, 25.0),
        phase: PointerPhase::PressedThisFrame,
        element_id: Some("left".into()),
        mouse_button: None,
        gesture: rlay::PointerGesture::Tap,
    }));
    assert!(result.pointers.contains(&PointerHit {
        pointer_id: PointerId::Touch(20),
        position: Point::new(125.0, 25.0),
        phase: PointerPhase::PressedThisFrame,
        element_id: Some("right".into()),
        mouse_button: None,
        gesture: rlay::PointerGesture::Tap,
    }));
}

#[test]
fn captured_pointer_keeps_target_when_dragging_outside() {
    let root = Node::new().child(Node::new().id("slider").layout(Layout {
        sizing: Sizing::fixed(50.0, 50.0),
        ..Layout::default()
    }));
    let mut engine = engine();
    engine
        .input_mut()
        .set_touch(1, Point::new(10.0, 10.0), true);
    engine
        .input_mut()
        .capture_pointer(PointerId::Touch(1), "slider");
    engine
        .input_mut()
        .set_touch(1, Point::new(90.0, 90.0), true);

    let result = engine.layout(&root, Size::new(100.0, 100.0), 0.0);

    assert_eq!(result.pointers[0].element_id.as_deref(), Some("slider"));
}

#[test]
fn pinch_reports_center_and_scale() {
    let mut engine = engine();
    engine.input_mut().set_touch(1, Point::new(0.0, 0.0), true);
    engine.input_mut().set_touch(2, Point::new(10.0, 0.0), true);
    engine.layout(&Node::new(), Size::new(100.0, 100.0), 0.0);
    engine.input_mut().set_touch(1, Point::new(-5.0, 0.0), true);
    engine.input_mut().set_touch(2, Point::new(15.0, 0.0), true);

    let pinch = engine.input_mut().pinch().unwrap();

    assert_eq!(pinch.center, Point::new(5.0, 0.0));
    assert_eq!(pinch.previous_center, Point::new(5.0, 0.0));
    assert_eq!(pinch.scale, 2.0);
}

#[test]
fn text_cursor_index_uses_measured_prefix_widths() {
    let mut engine = engine();
    let style = TextStyle::default();

    assert_eq!(engine.text_cursor_index_at_x("abcd", &style, 0.0), 0);
    assert_eq!(engine.text_cursor_index_at_x("abcd", &style, 17.0), 1);
    assert_eq!(engine.text_cursor_index_at_x("abcd", &style, 64.0), 4);
}
