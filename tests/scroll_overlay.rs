use rlay::{
    AxisSize, Color, CommandKind, Direction, Engine, Layout, Node, Point, PointerId, Rect,
    RenderCommand, Size, Sizing, Vector,
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
fn scroll_container_offsets_children_and_reports_data() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().id("row1").layout(Layout {
            sizing: Sizing::fixed(100.0, 40.0),
            ..Layout::default()
        }))
        .child(Node::new().id("row2").layout(Layout {
            sizing: Sizing::fixed(100.0, 40.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    engine.set_scroll_offset("list", Vector::new(0.0, 20.0));

    let result = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    assert_eq!(
        result.element("row1").unwrap().bounds,
        Rect::new(0.0, -20.0, 100.0, 40.0)
    );
    assert_eq!(
        result.element("row2").unwrap().bounds,
        Rect::new(0.0, 20.0, 100.0, 40.0)
    );
    assert_eq!(
        result.scroll_container("list").unwrap().content_size,
        Size::new(100.0, 80.0)
    );
    assert_eq!(
        result.scroll_container("list").unwrap().offset,
        Vector::new(0.0, 20.0)
    );
    assert!(matches!(
        result.commands[0],
        RenderCommand {
            id: Some(_),
            kind: CommandKind::ClipStart { x: false, y: true },
            ..
        }
    ));
}

#[test]
fn scroll_container_content_size_includes_padding() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            padding: rlay::Padding::all(12.0),
            ..Layout::default()
        })
        .child(Node::new().layout(Layout {
            sizing: Sizing::fixed(100.0, 100.0),
            ..Layout::default()
        }));
    let result = engine().layout(&root, Size::new(100.0, 80.0), 0.0);
    let scroll = result.scroll_container("list").unwrap();

    assert_eq!(scroll.content_size, Size::new(100.0, 124.0));
}

#[test]
fn culling_skips_offscreen_commands_but_keeps_element_data() {
    let root = Node::new()
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(
            Node::new()
                .id("visible")
                .background(Color::rgba(1.0, 1.0, 1.0, 255.0))
                .layout(Layout {
                    sizing: Sizing::fixed(10.0, 10.0),
                    ..Layout::default()
                }),
        )
        .child(
            Node::new()
                .id("hidden")
                .background(Color::rgba(2.0, 2.0, 2.0, 255.0))
                .layout(Layout {
                    sizing: Sizing {
                        width: AxisSize::fixed(10.0),
                        height: AxisSize::fixed(10.0),
                    },
                    ..Layout::default()
                }),
        );
    let mut engine = engine();
    engine.set_culling(true);

    let result = engine.layout(&root, Size::new(10.0, 10.0), 0.0);

    assert!(result.element("hidden").is_some());
    assert_eq!(result.commands.len(), 1);
    assert_eq!(result.commands[0].id.as_deref(), Some("visible"));
}

#[test]
fn overlay_wraps_child_commands() {
    let overlay = Color::rgba(255.0, 0.0, 0.0, 80.0);
    let root = Node::new().id("panel").overlay(overlay).child(
        Node::new()
            .id("child")
            .background(Color::rgba(1.0, 1.0, 1.0, 255.0))
            .layout(Layout {
                sizing: Sizing::fixed(10.0, 10.0),
                ..Layout::default()
            }),
    );

    let result = engine().layout(&root, Size::new(20.0, 20.0), 0.0);

    assert!(
        matches!(result.commands[0].kind, CommandKind::OverlayStart(color) if color == overlay)
    );
    assert!(matches!(
        result.commands[1].kind,
        CommandKind::Rectangle { .. }
    ));
    assert!(matches!(result.commands[2].kind, CommandKind::OverlayEnd));
}

#[test]
fn wheel_scroll_updates_next_frame_offset() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().layout(Layout {
            sizing: Sizing::fixed(100.0, 100.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    engine
        .input_mut()
        .set_mouse_position(Point::new(10.0, 10.0));
    engine.input_mut().add_scroll_delta_with_phase(
        PointerId::Mouse,
        Vector::new(0.0, 30.0),
        Some(rlay::TouchPhase::Moved),
    );

    let first = engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    let second = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    assert_eq!(first.scroll_container("list").unwrap().offset, Vector::ZERO);
    assert_eq!(
        second.scroll_container("list").unwrap().offset,
        Vector::new(0.0, 30.0)
    );
}

#[test]
fn wheel_momentum_starts_after_wheel_phase_ends() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().layout(Layout {
            sizing: Sizing::fixed(100.0, 200.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    engine
        .input_mut()
        .set_mouse_position(Point::new(10.0, 10.0));
    engine.input_mut().add_scroll_delta_with_phase(
        PointerId::Mouse,
        Vector::new(0.0, 30.0),
        Some(rlay::TouchPhase::Moved),
    );

    engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    let active = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    engine.input_mut().add_scroll_delta_with_phase(
        PointerId::Mouse,
        Vector::ZERO,
        Some(rlay::TouchPhase::Ended),
    );
    let ended = engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    let momentum = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    assert_eq!(
        active.scroll_container("list").unwrap().offset,
        Vector::new(0.0, 30.0)
    );
    assert_eq!(
        ended.scroll_container("list").unwrap().offset,
        Vector::new(0.0, 30.0)
    );
    assert!(momentum.scroll_container("list").unwrap().offset.y > 30.0);
}

#[test]
fn wheel_started_resets_existing_momentum() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().layout(Layout {
            sizing: Sizing::fixed(100.0, 200.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    engine
        .input_mut()
        .set_mouse_position(Point::new(10.0, 10.0));
    engine.set_scroll_momentum("list", Vector::new(0.0, 40.0));
    engine.input_mut().add_scroll_delta_with_phase(
        PointerId::Mouse,
        Vector::new(0.0, 10.0),
        Some(rlay::TouchPhase::Started),
    );

    engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    let active = engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    engine.input_mut().add_scroll_delta_with_phase(
        PointerId::Mouse,
        Vector::ZERO,
        Some(rlay::TouchPhase::Ended),
    );
    let ended = engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    let momentum = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    assert_eq!(
        active.scroll_container("list").unwrap().offset,
        Vector::new(0.0, 10.0)
    );
    assert_eq!(
        ended.scroll_container("list").unwrap().offset,
        Vector::new(0.0, 10.0)
    );
    assert!(momentum.scroll_container("list").unwrap().offset.y > 10.0);
}

#[test]
fn touch_drag_can_overscroll_past_top() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().layout(Layout {
            sizing: Sizing::fixed(100.0, 100.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    let previous = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    engine
        .input_mut()
        .set_touch(1, Point::new(10.0, 10.0), true);
    engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    engine
        .input_mut()
        .set_touch(1, Point::new(10.0, 40.0), true);
    engine.apply_input_scroll(&previous);

    let current = engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    let offset = current.scroll_container("list").unwrap().offset.y;

    assert_eq!(offset, -15.0);
}

#[test]
fn touch_drag_tracks_finger_across_bottom_and_back() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().layout(Layout {
            sizing: Sizing::fixed(100.0, 100.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    engine.set_scroll_offset("list", Vector::new(0.0, 30.0));
    let mut previous = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    engine
        .input_mut()
        .set_touch(1, Point::new(10.0, 50.0), true);
    engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    for (finger_y, expected_offset) in [(20.0, 55.0), (35.0, 45.0), (10.0, 60.0)] {
        engine
            .input_mut()
            .set_touch(1, Point::new(10.0, finger_y), true);
        engine.apply_input_scroll(&previous);
        previous = engine.layout(&root, Size::new(100.0, 50.0), 0.0);
        assert_eq!(
            previous.scroll_container("list").unwrap().offset.y,
            expected_offset
        );
    }
}

#[test]
fn touch_drag_can_grab_returning_overscroll_without_jumping() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().layout(Layout {
            sizing: Sizing::fixed(100.0, 100.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    let mut previous = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    engine
        .input_mut()
        .set_touch(1, Point::new(10.0, 10.0), true);
    engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    engine
        .input_mut()
        .set_touch(1, Point::new(10.0, 40.0), true);
    engine.apply_input_scroll(&previous);
    engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    engine.input_mut().remove_touch(1, Point::new(10.0, 40.0));
    engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    for _ in 0..2 {
        engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    }

    engine
        .input_mut()
        .set_touch(2, Point::new(10.0, 40.0), true);
    previous = engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    let before = previous.scroll_container("list").unwrap().offset.y;
    engine
        .input_mut()
        .set_touch(2, Point::new(10.0, 47.0), true);
    engine.apply_input_scroll(&previous);
    let after = engine
        .layout(&root, Size::new(100.0, 50.0), 0.0)
        .scroll_container("list")
        .unwrap()
        .offset
        .y;

    assert_eq!(after, before - 3.5);
}

#[test]
fn touchpad_phase_can_overscroll_past_top() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().layout(Layout {
            sizing: Sizing::fixed(100.0, 100.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    engine
        .input_mut()
        .set_mouse_position(Point::new(10.0, 10.0));
    engine.input_mut().add_scroll_delta_with_phase(
        PointerId::Mouse,
        Vector::new(0.0, -30.0),
        Some(rlay::TouchPhase::Moved),
    );

    engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    let current = engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    let offset = current.scroll_container("list").unwrap().offset.y;

    assert_eq!(offset, -15.0);
}

#[test]
fn touchpad_overscroll_stays_held_until_phase_ends() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().layout(Layout {
            sizing: Sizing::fixed(100.0, 100.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    engine
        .input_mut()
        .set_mouse_position(Point::new(10.0, 10.0));
    engine.input_mut().add_scroll_delta_with_phase(
        PointerId::Mouse,
        Vector::new(0.0, -30.0),
        Some(rlay::TouchPhase::Started),
    );

    engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    let held = engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    let held_y = held.scroll_container("list").unwrap().offset.y;

    for _ in 0..5 {
        let frame = engine.layout(&root, Size::new(100.0, 50.0), 0.0);
        assert_eq!(frame.scroll_container("list").unwrap().offset.y, held_y);
    }

    engine.input_mut().add_scroll_delta_with_phase(
        PointerId::Mouse,
        Vector::ZERO,
        Some(rlay::TouchPhase::Ended),
    );
    engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    let first_released = engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    assert_eq!(
        first_released.scroll_container("list").unwrap().offset.y,
        held_y
    );

    for _ in 0..80 {
        engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    }
    let released = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    assert!(released.scroll_container("list").unwrap().offset.y > held_y);
}

#[test]
fn touchpad_restart_during_overscroll_does_not_jump_back() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().layout(Layout {
            sizing: Sizing::fixed(100.0, 100.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    engine.set_scroll_offset("list", Vector::new(0.0, 50.0));
    engine
        .input_mut()
        .set_mouse_position(Point::new(10.0, 10.0));
    let mut previous = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    for (phase, physical_y) in [
        (rlay::TouchPhase::Started, -11.964_844),
        (rlay::TouchPhase::Moved, -12.132_813),
        (rlay::TouchPhase::Moved, -12.558_594),
        (rlay::TouchPhase::Moved, -13.40625),
        (rlay::TouchPhase::Moved, -14.679_688),
        (rlay::TouchPhase::Moved, -14.59375),
        (rlay::TouchPhase::Moved, -16.632_813),
        (rlay::TouchPhase::Moved, -16.460_938),
        (rlay::TouchPhase::Moved, -15.445_313),
        (rlay::TouchPhase::Moved, -14.253_906),
        (rlay::TouchPhase::Moved, -12.21875),
        (rlay::TouchPhase::Moved, -10.859_375),
        (rlay::TouchPhase::Ended, -0.0),
    ] {
        engine.input_mut().add_scroll_delta_with_phase(
            PointerId::Mouse,
            Vector::new(0.0, -physical_y),
            Some(phase),
        );
        engine.apply_input_scroll(&previous);
        previous = engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    }

    for _ in 0..3 {
        previous = engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    }
    let before_restart = previous.scroll_container("list").unwrap().offset.y;

    engine.input_mut().add_scroll_delta_with_phase(
        PointerId::Mouse,
        Vector::new(0.0, 10.945_313),
        Some(rlay::TouchPhase::Started),
    );
    engine.apply_input_scroll(&previous);
    let restarted = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    assert!(restarted.scroll_container("list").unwrap().offset.y >= before_restart);
}

#[test]
fn scroll_momentum_overshoots_then_springs_back() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().layout(Layout {
            sizing: Sizing::fixed(100.0, 100.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    engine.set_scroll_offset("list", Vector::new(0.0, 45.0));
    engine.set_scroll_momentum("list", Vector::new(0.0, 20.0));

    engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    let overshot = engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    let overshot_y = overshot.scroll_container("list").unwrap().offset.y;

    for _ in 0..160 {
        engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    }
    let settled = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    assert!(overshot_y > 50.0);
    assert_eq!(
        settled.scroll_container("list").unwrap().offset,
        Vector::new(0.0, 50.0)
    );
}

#[test]
fn overscroll_velocity_accelerates_towards_distance_target() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().layout(Layout {
            sizing: Sizing::fixed(100.0, 100.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    engine.set_scroll_offset("list", Vector::new(0.0, -100.0));
    engine.set_scroll_momentum("list", Vector::new(0.0, 5.0));

    let before = engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    let after = engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    let moved = after.scroll_container("list").unwrap().offset.y
        - before.scroll_container("list").unwrap().offset.y;

    assert!(moved > 5.0);
    assert!(moved < 10.0);
}

#[test]
fn overscroll_damps_extreme_inertia_without_stopping_immediately() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().layout(Layout {
            sizing: Sizing::fixed(100.0, 100.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    engine.set_scroll_offset("list", Vector::new(0.0, -20.0));
    engine.set_scroll_momentum("list", Vector::new(0.0, -100.0));

    let before = engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    let after = engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    let moved = after.scroll_container("list").unwrap().offset.y
        - before.scroll_container("list").unwrap().offset.y;

    assert!(moved < 0.0);
    assert!(moved > -100.0);
}

#[test]
fn touch_drag_scrolls_content_on_next_frame() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().layout(Layout {
            sizing: Sizing::fixed(100.0, 100.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    engine
        .input_mut()
        .set_touch(1, Point::new(10.0, 30.0), true);
    engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    engine
        .input_mut()
        .set_touch(1, Point::new(10.0, 10.0), true);

    let before = engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    let after = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    assert_eq!(
        before.scroll_container("list").unwrap().offset,
        Vector::ZERO
    );
    assert_eq!(
        after.scroll_container("list").unwrap().offset,
        Vector::new(0.0, 20.0)
    );
}

#[test]
fn mouse_drag_does_not_scroll_content() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().layout(Layout {
            sizing: Sizing::fixed(100.0, 100.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    engine
        .input_mut()
        .set_mouse_down(Point::new(10.0, 30.0), true);
    engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    engine
        .input_mut()
        .set_mouse_down(Point::new(10.0, 10.0), true);

    engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    let after = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    assert_eq!(after.scroll_container("list").unwrap().offset, Vector::ZERO);
}

#[test]
fn touch_drag_can_update_scroll_before_current_layout() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().layout(Layout {
            sizing: Sizing::fixed(100.0, 100.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    let previous = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    engine
        .input_mut()
        .set_touch(1, Point::new(10.0, 30.0), true);
    engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    engine
        .input_mut()
        .set_touch(1, Point::new(10.0, 10.0), true);

    engine.apply_input_scroll(&previous);
    let current = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    assert_eq!(
        current.scroll_container("list").unwrap().offset,
        Vector::new(0.0, 20.0)
    );
}

#[test]
fn touch_scroll_applies_full_drag_when_gesture_wins() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().layout(Layout {
            sizing: Sizing::fixed(100.0, 100.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    let previous = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    engine
        .input_mut()
        .set_touch(1, Point::new(10.0, 30.0), true);
    engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    engine
        .input_mut()
        .set_touch(1, Point::new(10.0, 23.0), true);

    engine.apply_input_scroll(&previous);
    let current = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    assert_eq!(
        current.scroll_container("list").unwrap().offset,
        Vector::new(0.0, 7.0)
    );
}

#[test]
fn vertical_touch_scroll_ignores_horizontal_drag_slop() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().layout(Layout {
            sizing: Sizing::fixed(100.0, 100.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    let previous = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    engine
        .input_mut()
        .set_touch(1, Point::new(10.0, 30.0), true);
    engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    engine
        .input_mut()
        .set_touch(1, Point::new(30.0, 32.0), true);

    engine.apply_input_scroll(&previous);
    let current = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    assert_eq!(
        current.scroll_container("list").unwrap().offset,
        Vector::ZERO
    );
}

#[test]
fn horizontal_touch_scroll_ignores_vertical_drag_slop() {
    let root = Node::new()
        .id("list")
        .scroll(true, false)
        .layout(Layout {
            direction: Direction::Row,
            ..Layout::default()
        })
        .child(Node::new().layout(Layout {
            sizing: Sizing::fixed(100.0, 100.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    let previous = engine.layout(&root, Size::new(50.0, 100.0), 0.0);

    engine
        .input_mut()
        .set_touch(1, Point::new(30.0, 10.0), true);
    engine.layout(&root, Size::new(50.0, 100.0), 0.0);
    engine
        .input_mut()
        .set_touch(1, Point::new(32.0, 30.0), true);

    engine.apply_input_scroll(&previous);
    let current = engine.layout(&root, Size::new(50.0, 100.0), 0.0);

    assert_eq!(
        current.scroll_container("list").unwrap().offset,
        Vector::ZERO
    );
}

#[test]
fn two_finger_drag_does_not_scroll_twice_as_fast() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().layout(Layout {
            sizing: Sizing::fixed(100.0, 100.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    engine
        .input_mut()
        .set_touch(1, Point::new(10.0, 30.0), true);
    engine
        .input_mut()
        .set_touch(2, Point::new(20.0, 30.0), true);
    engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    engine
        .input_mut()
        .set_touch(1, Point::new(10.0, 10.0), true);
    engine.input_mut().set_touch(2, Point::new(20.0, 0.0), true);

    engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    let after = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    assert_eq!(
        after.scroll_container("list").unwrap().offset,
        Vector::new(0.0, 20.0)
    );
}

#[test]
fn touch_scroll_momentum_continues_after_release() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().layout(Layout {
            sizing: Sizing::fixed(100.0, 200.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    engine
        .input_mut()
        .set_touch(1, Point::new(10.0, 80.0), true);
    engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    engine
        .input_mut()
        .set_touch(1, Point::new(10.0, 40.0), true);
    engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    engine.input_mut().remove_touch(1, Point::new(10.0, 40.0));

    let before_momentum = engine.layout(&root, Size::new(100.0, 50.0), 0.0);
    let after_momentum = engine.layout(&root, Size::new(100.0, 50.0), 0.0);

    assert!(
        after_momentum.scroll_container("list").unwrap().offset.y
            > before_momentum.scroll_container("list").unwrap().offset.y
    );
}
