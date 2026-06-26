use crate::*;
use std::{cell::Cell, rc::Rc};

fn assert_close(left: f32, right: f32) {
    assert!((left - right).abs() <= f32::EPSILON);
}

fn engine() -> Engine {
    Engine::new(|text, style| {
        Size::new(
            text.chars().count() as f32 * style.font_size,
            style.font_size,
        )
    })
}

#[test]
fn lays_out_row_with_fixed_and_grow_children() {
    let root = Node::new()
        .layout(Layout {
            sizing: Sizing::fixed(300.0, 40.0),
            direction: Direction::Row,
            gap: 10.0,
            ..Layout::default()
        })
        .child(Node::new().id("left").layout(Layout {
            sizing: Sizing::fixed(50.0, 20.0),
            ..Layout::default()
        }))
        .child(Node::new().id("right").layout(Layout {
            sizing: Sizing {
                width: AxisSize::GROW,
                height: AxisSize::fixed(20.0),
            },
            ..Layout::default()
        }));

    let result = engine().layout(&root, Size::new(300.0, 40.0), 0.0);

    assert_eq!(
        result.elements["left"].bounds,
        Rect::new(0.0, 0.0, 50.0, 20.0)
    );
    assert_eq!(
        result.elements["right"].bounds,
        Rect::new(60.0, 0.0, 240.0, 20.0)
    );
}

#[test]
fn text_uses_measure_callback_for_fit_size() {
    let root = Node::new().child(Node::text("abc", TextStyle::default()).id("text"));

    let result = engine().layout(&root, Size::new(300.0, 40.0), 0.0);

    assert_close(result.elements["text"].bounds.width, 48.0);
    assert_close(result.elements["text"].bounds.height, 16.0);
}

#[test]
fn idle_transitions_do_not_run_a_second_layout_pass() {
    fn measure_calls_for(root: &Node) -> usize {
        let calls = Rc::new(Cell::new(0));
        let calls_for_measure = Rc::clone(&calls);
        let mut engine = Engine::new(move |text, style| {
            calls_for_measure.set(calls_for_measure.get() + 1);
            Size::new(
                text.chars().count() as f32 * style.font_size,
                style.font_size,
            )
        });
        engine.set_max_measure_text_cache_entries(Some(0));

        engine.layout(root, Size::new(300.0, 40.0), 0.0);

        calls.get()
    }

    let plain = Node::new().child(
        Node::new()
            .id("card")
            .child(Node::text("abc", TextStyle::default()).id("text")),
    );
    let with_idle_transition = Node::new().child(
        Node::new()
            .id("card")
            .transition(Transition::ease_out(0.2, TransitionProperties::POSITION))
            .child(Node::text("abc", TextStyle::default()).id("text")),
    );

    assert_eq!(
        measure_calls_for(&with_idle_transition),
        measure_calls_for(&plain)
    );
}

#[test]
fn active_transition_requests_animation_frame() {
    let mut engine = engine();
    let root = Node::new().child(
        Node::new()
            .id("panel")
            .transition(Transition::ease_out(1.0, TransitionProperties::WIDTH))
            .layout(Layout {
                sizing: Sizing::fixed(10.0, 10.0),
                ..Layout::default()
            }),
    );
    assert!(!engine.layout(&root, Size::new(100.0, 100.0), 0.0).needs_animation_frame);

    let root = Node::new().child(
        Node::new()
            .id("panel")
            .transition(Transition::ease_out(1.0, TransitionProperties::WIDTH))
            .layout(Layout {
                sizing: Sizing::fixed(20.0, 10.0),
                ..Layout::default()
            }),
    );
    assert!(engine.layout(&root, Size::new(100.0, 100.0), 0.0).needs_animation_frame);
}

#[test]
fn static_layout_does_not_request_animation_frame() {
    let root = Node::new().child(Node::new().id("panel").layout(Layout {
        sizing: Sizing::fixed(10.0, 10.0),
        ..Layout::default()
    }));
    assert!(!engine().layout(&root, Size::new(100.0, 100.0), 0.0).needs_animation_frame);
}

#[test]
fn hit_test_returns_matching_element_id() {
    let root = Node::new().child(Node::new().id("button").layout(Layout {
        sizing: Sizing::fixed(100.0, 40.0),
        ..Layout::default()
    }));
    let mut engine = engine();
    let result = engine.layout(&root, Size::new(300.0, 200.0), 0.0);

    assert_eq!(
        Engine::hit_test(&result, Point::new(20.0, 20.0)),
        Some("button")
    );
    assert_eq!(Engine::hit_test(&result, Point::new(120.0, 20.0)), None);
}

#[test]
fn hit_test_prefers_later_overlapping_elements() {
    let root = Node::new()
        .id("back")
        .child(Node::new().id("front").layout(Layout {
            sizing: Sizing::fixed(100.0, 40.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    let result = engine.layout(&root, Size::new(100.0, 40.0), 0.0);

    assert_eq!(
        Engine::hit_test(&result, Point::new(20.0, 20.0)),
        Some("front")
    );
}

#[test]
fn padding_and_gap_offset_children() {
    let root = Node::new()
        .layout(Layout {
            padding: Padding {
                left: 10.0,
                right: 2.0,
                top: 5.0,
                bottom: 1.0,
            },
            gap: 3.0,
            ..Layout::default()
        })
        .child(Node::new().id("a").layout(Layout {
            sizing: Sizing::fixed(20.0, 10.0),
            ..Layout::default()
        }))
        .child(Node::new().id("b").layout(Layout {
            sizing: Sizing::fixed(30.0, 10.0),
            ..Layout::default()
        }));

    let result = engine().layout(&root, Size::new(100.0, 40.0), 0.0);

    assert_eq!(
        result.elements["a"].bounds,
        Rect::new(10.0, 5.0, 20.0, 10.0)
    );
    assert_eq!(
        result.elements["b"].bounds,
        Rect::new(33.0, 5.0, 30.0, 10.0)
    );
}

#[test]
fn center_alignment_moves_children_on_both_axes() {
    let root = Node::new()
        .layout(Layout {
            direction: Direction::Row,
            align_x: AlignX::Center,
            align_y: AlignY::Center,
            ..Layout::default()
        })
        .child(Node::new().id("child").layout(Layout {
            sizing: Sizing::fixed(20.0, 10.0),
            ..Layout::default()
        }));

    let result = engine().layout(&root, Size::new(100.0, 50.0), 0.0);

    assert_eq!(
        result.elements["child"].bounds,
        Rect::new(40.0, 20.0, 20.0, 10.0)
    );
}

#[test]
fn column_grow_uses_vertical_space_only() {
    let root = Node::new()
        .layout(Layout {
            direction: Direction::Column,
            gap: 5.0,
            ..Layout::default()
        })
        .child(Node::new().id("top").layout(Layout {
            sizing: Sizing::fixed(40.0, 10.0),
            ..Layout::default()
        }))
        .child(Node::new().id("fill").layout(Layout {
            sizing: Sizing {
                width: AxisSize::fixed(40.0),
                height: AxisSize::GROW,
            },
            ..Layout::default()
        }));

    let result = engine().layout(&root, Size::new(100.0, 80.0), 0.0);

    assert_eq!(
        result.elements["top"].bounds,
        Rect::new(0.0, 0.0, 40.0, 10.0)
    );
    assert_eq!(
        result.elements["fill"].bounds,
        Rect::new(0.0, 15.0, 40.0, 65.0)
    );
}

#[test]
fn percent_resolves_against_own_axis() {
    let root = Node::new().child(Node::new().id("child").layout(Layout {
        sizing: Sizing {
            width: AxisSize::Percent(0.5),
            height: AxisSize::Percent(0.25),
        },
        ..Layout::default()
    }));

    let result = engine().layout(&root, Size::new(200.0, 80.0), 0.0);

    assert_eq!(
        result.elements["child"].bounds,
        Rect::new(0.0, 0.0, 100.0, 20.0)
    );
}

#[test]
fn render_commands_preserve_paint_order() {
    let root = Node::new()
        .background(Color::rgba(1.0, 2.0, 3.0, 255.0))
        .child(Node::text("ok", TextStyle::default()).id("label"));

    let result = engine().layout(&root, Size::new(100.0, 40.0), 0.0);

    assert!(matches!(
        result.commands[0].kind,
        CommandKind::Rectangle { .. }
    ));
    assert!(matches!(result.commands[1].kind, CommandKind::Text { .. }));
}

#[test]
fn image_command_matches_clay_render_data() {
    let color = Color::rgba(10.0, 20.0, 30.0, 40.0);
    let radius = Radius::all(8.0);
    let root = Node::image(ImageId::new(42))
        .background(color)
        .radius(radius)
        .layout(Layout {
            sizing: Sizing::fixed(120.0, 80.0),
            ..Layout::default()
        });

    let result = engine().layout(&root, Size::new(120.0, 80.0), 0.0);
    let image = result
        .commands
        .iter()
        .find_map(|command| match command.kind {
            CommandKind::Image(image) => Some(image),
            _ => None,
        })
        .expect("image command");

    assert_eq!(image.image_id, ImageId::new(42));
    assert_eq!(image.background_color, color);
    assert_eq!(image.corner_radius, radius);
}

#[test]
fn image_aspect_ratio_derives_fit_height_from_fixed_width() {
    let root = Node::new().child(Node::image(1).id("image").aspect_ratio(2.0).layout(Layout {
        sizing: Sizing {
            width: AxisSize::fixed(120.0),
            height: AxisSize::FIT,
        },
        ..Layout::default()
    }));

    let result = engine().layout(&root, Size::new(300.0, 200.0), 0.0);

    assert_eq!(
        result.elements["image"].bounds,
        Rect::new(0.0, 0.0, 120.0, 60.0)
    );
}

#[test]
fn image_aspect_ratio_derives_fit_width_before_sibling_layout() {
    let root = Node::new()
        .layout(Layout {
            direction: Direction::Row,
            ..Layout::default()
        })
        .child(Node::image(1).id("image").aspect_ratio(2.0).layout(Layout {
            sizing: Sizing {
                width: AxisSize::FIT,
                height: AxisSize::fixed(60.0),
            },
            ..Layout::default()
        }))
        .child(Node::new().id("sibling").layout(Layout {
            sizing: Sizing::fixed(20.0, 20.0),
            ..Layout::default()
        }));

    let result = engine().layout(&root, Size::new(300.0, 200.0), 0.0);

    assert_eq!(
        result.elements["image"].bounds,
        Rect::new(0.0, 0.0, 120.0, 60.0)
    );
    assert_close(result.elements["sibling"].bounds.x, 120.0);
}

#[test]
fn reports_multiple_touch_hits_in_same_frame() {
    let root = Node::new()
        .layout(Layout {
            direction: Direction::Row,
            ..Layout::default()
        })
        .child(Node::new().id("left").layout(Layout {
            sizing: Sizing::fixed(50.0, 50.0),
            ..Layout::default()
        }))
        .child(Node::new().id("right").layout(Layout {
            sizing: Sizing::fixed(50.0, 50.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    engine
        .input_mut()
        .set_touch(1, Point::new(10.0, 10.0), true);
    engine
        .input_mut()
        .set_touch(2, Point::new(60.0, 10.0), true);

    let result = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    assert!(result.pointers.contains(&PointerHit {
        pointer_id: PointerId::Touch(1),
        position: Point::new(10.0, 10.0),
        phase: PointerPhase::PressedThisFrame,
        element_id: Some("left".into()),
        mouse_button: None,
        gesture: PointerGesture::Tap,
    }));
    assert!(result.pointers.contains(&PointerHit {
        pointer_id: PointerId::Touch(2),
        position: Point::new(60.0, 10.0),
        phase: PointerPhase::PressedThisFrame,
        element_id: Some("right".into()),
        mouse_button: None,
        gesture: PointerGesture::Tap,
    }));
}

#[test]
fn pointer_phase_advances_after_layout_frame() {
    let root = Node::new().id("root");
    let mut engine = engine();
    engine
        .input_mut()
        .set_mouse_down(Point::new(1.0, 1.0), true);
    let first = engine.layout(&root, Size::new(10.0, 10.0), 0.0);
    let second = engine.layout(&root, Size::new(10.0, 10.0), 0.0);
    engine
        .input_mut()
        .set_mouse_down(Point::new(1.0, 1.0), false);
    let third = engine.layout(&root, Size::new(10.0, 10.0), 0.0);

    assert_eq!(first.pointers[0].phase, PointerPhase::PressedThisFrame);
    assert_eq!(second.pointers[0].phase, PointerPhase::Pressed);
    assert_eq!(third.pointers[0].phase, PointerPhase::ReleasedThisFrame);
}

#[test]
fn layout_result_exposes_pointer_over_queries() {
    let root = Node::new().child(Node::new().id("button").layout(Layout {
        sizing: Sizing::fixed(40.0, 40.0),
        ..Layout::default()
    }));
    let mut engine = engine();
    engine
        .input_mut()
        .set_mouse_position(Point::new(10.0, 10.0));

    let result = engine.layout(&root, Size::new(100.0, 100.0), 0.0);

    assert_eq!(
        result.element("button").unwrap().bounds,
        Rect::new(0.0, 0.0, 40.0, 40.0)
    );
    assert!(result.pointer_over("button"));
    assert_eq!(result.pointer_over_ids(), vec!["button"]);
}

#[test]
fn current_input_can_hit_test_against_previous_layout() {
    let root = Node::new().child(Node::new().id("button").layout(Layout {
        sizing: Sizing::fixed(40.0, 40.0),
        ..Layout::default()
    }));
    let mut engine = engine();
    let previous = engine.layout(&root, Size::new(100.0, 100.0), 0.0);

    engine
        .input_mut()
        .set_mouse_button(Point::new(10.0, 10.0), MouseButton::Right, true);
    engine
        .input_mut()
        .set_touch(7, Point::new(10.0, 10.0), true);

    let hits = previous.pointer_hits(engine.input());

    assert!(hits.contains(&PointerHit {
        pointer_id: PointerId::Mouse,
        position: Point::new(10.0, 10.0),
        phase: PointerPhase::PressedThisFrame,
        element_id: Some("button".into()),
        mouse_button: Some(MouseButton::Right),
        gesture: PointerGesture::Tap,
    }));
    assert!(hits.contains(&PointerHit {
        pointer_id: PointerId::Touch(7),
        position: Point::new(10.0, 10.0),
        phase: PointerPhase::PressedThisFrame,
        element_id: Some("button".into()),
        mouse_button: None,
        gesture: PointerGesture::Tap,
    }));
}
