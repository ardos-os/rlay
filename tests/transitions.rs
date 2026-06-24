use rlay::{
    AxisSize, Color, CommandKind, Direction, Engine, Layout, LayoutError, Node, Padding, Point,
    Radius, Rect, Size, Sizing, Transition, TransitionArgs, TransitionEnter,
    TransitionEnterTrigger, TransitionExit, TransitionExitOrdering, TransitionFrame,
    TransitionInteraction, TransitionProperties, TransitionState, TransitionValues, ease_out,
};

fn engine() -> Engine {
    Engine::new(|_, style| Size::new(0.0, style.font_size))
}

fn transition(properties: TransitionProperties) -> Transition {
    Transition::ease_out(1.0, properties)
}

fn box_node(id: &str, width: f32, transition: Transition) -> Node {
    Node::new()
        .id(id)
        .layout(Layout {
            sizing: Sizing::fixed(width, 10.0),
            ..Layout::default()
        })
        .transition(transition)
}

fn offset_y(mut values: TransitionValues, properties: TransitionProperties) -> TransitionValues {
    if properties.contains(TransitionProperties::Y) {
        values.bounds.y += 20.0;
    }
    values
}

fn rectangle(result: &rlay::LayoutResult, id: &str) -> (Color, Radius) {
    result
        .commands
        .iter()
        .find_map(|command| match command {
            rlay::RenderCommand {
                id: Some(command_id),
                kind: CommandKind::Rectangle { color, radius },
                ..
            } if command_id == id => Some((*color, *radius)),
            _ => None,
        })
        .unwrap()
}

#[test]
fn ease_out_matches_clay_for_all_value_types() {
    let initial = TransitionValues {
        bounds: Rect::new(0.0, 0.0, 10.0, 20.0),
        background: Color::rgba(0.0, 10.0, 20.0, 30.0),
        overlay: Color::rgba(10.0, 20.0, 30.0, 40.0),
        radius: Radius::all(0.0),
        border_color: Color::rgba(20.0, 30.0, 40.0, 50.0),
        border_width: Padding::all(0.0),
    };
    let target = TransitionValues {
        bounds: Rect::new(8.0, 16.0, 18.0, 28.0),
        background: Color::rgba(8.0, 18.0, 28.0, 38.0),
        overlay: Color::rgba(18.0, 28.0, 38.0, 48.0),
        radius: Radius::all(8.0),
        border_color: Color::rgba(28.0, 38.0, 48.0, 58.0),
        border_width: Padding::all(8.0),
    };
    let TransitionFrame { values, complete } = ease_out(TransitionArgs {
        state: TransitionState::Transitioning,
        initial,
        current: initial,
        target,
        elapsed: 0.5,
        duration: 1.0,
        properties: TransitionProperties::all(),
    });

    assert_eq!(values.bounds, Rect::new(7.0, 14.0, 17.0, 27.0));
    assert_eq!(values.background.r, 7.0);
    assert_eq!(values.overlay.r, 17.0);
    assert_eq!(values.radius, Radius::all(7.0));
    assert_eq!(values.border_color.r, 27.0);
    assert_eq!(values.border_width, Padding::all(7.0));
    assert!(!complete);
}

#[test]
fn retarget_has_no_jump_and_unselected_paint_snaps() {
    let transition = transition(TransitionProperties::WIDTH);
    let node = |width, red| {
        box_node("box", width, transition).background(Color::rgba(red, 0.0, 0.0, 255.0))
    };
    let mut engine = engine();

    let root = |width, red| Node::new().child(node(width, red));
    engine.layout(&root(10.0, 0.0), Size::new(100.0, 20.0), 0.0);
    engine.layout(&root(20.0, 100.0), Size::new(100.0, 20.0), 0.5);
    let midway = engine.layout(&root(20.0, 100.0), Size::new(100.0, 20.0), 0.0);
    let restarted = engine.layout(&root(30.0, 200.0), Size::new(100.0, 20.0), 0.0);

    assert_eq!(
        midway.element("box").unwrap().bounds,
        restarted.element("box").unwrap().bounds
    );
    assert_eq!(rectangle(&restarted, "box").0.r, 200.0);
}

#[test]
fn animated_width_participates_in_sibling_layout() {
    let root = |width| {
        Node::new()
            .layout(Layout {
                direction: Direction::Row,
                ..Layout::default()
            })
            .child(box_node(
                "box",
                width,
                transition(TransitionProperties::WIDTH),
            ))
            .child(Node::new().id("sibling").layout(Layout {
                sizing: Sizing::fixed(10.0, 10.0),
                ..Layout::default()
            }))
    };
    let mut engine = engine();

    engine.layout(&root(10.0), Size::new(100.0, 20.0), 0.0);
    engine.layout(&root(20.0), Size::new(100.0, 20.0), 0.5);
    let animated = engine.layout(&root(20.0), Size::new(100.0, 20.0), 0.0);

    assert_eq!(animated.element("box").unwrap().bounds.width, 18.75);
    assert_eq!(animated.element("sibling").unwrap().bounds.x, 18.75);
}

#[test]
fn removing_transition_stops_active_runtime() {
    let mut engine = engine();
    let root = |width, animated| {
        let node = Node::new().id("box").layout(Layout {
            sizing: Sizing::fixed(width, 10.0),
            ..Layout::default()
        });
        Node::new().child(if animated {
            node.transition(transition(TransitionProperties::WIDTH))
        } else {
            node
        })
    };

    engine.layout(&root(10.0, true), Size::new(100.0, 20.0), 0.0);
    engine.layout(&root(20.0, true), Size::new(100.0, 20.0), 0.5);
    let result = engine.layout(&root(30.0, false), Size::new(100.0, 20.0), 0.0);

    assert_eq!(result.element("box").unwrap().bounds.width, 30.0);
}

#[test]
fn exiting_child_does_not_consume_layout_space() {
    let item_transition = || Transition {
        enter: TransitionEnter {
            initial: Some(offset_y),
            trigger: TransitionEnterTrigger::OnFirstParentFrame,
        },
        exit: TransitionExit {
            target: Some(offset_y),
            ..TransitionExit::default()
        },
        ..transition(TransitionProperties::BOUNDS)
    };
    let row = |ids: &[&str]| {
        ids.iter().fold(
            Node::new().layout(Layout {
                direction: Direction::Row,
                ..Layout::default()
            }),
            |row, id| {
                row.child(
                    Node::new()
                        .id(*id)
                        .layout(Layout {
                            sizing: Sizing {
                                width: AxisSize::GROW,
                                height: AxisSize::fixed(10.0),
                            },
                            ..Layout::default()
                        })
                        .transition(item_transition()),
                )
            },
        )
    };
    let mut engine = engine();

    engine.layout(&row(&["a", "b", "c", "d"]), Size::new(100.0, 10.0), 0.0);
    let changed = engine.layout(&row(&["b", "c", "d", "e"]), Size::new(100.0, 10.0), 0.0);

    assert!(changed.element("a").is_some());
    assert_eq!(changed.element("e").unwrap().bounds.width, 25.0);
    assert_eq!(changed.element("e").unwrap().bounds.x, 75.0);
}

#[test]
fn paint_only_exit_preserves_position() {
    let exiting = Node::new()
        .id("exiting")
        .layout(Layout {
            sizing: Sizing::fixed(10.0, 10.0),
            ..Layout::default()
        })
        .background(Color::rgb(255.0, 255.0, 255.0))
        .transition(Transition {
            exit: TransitionExit {
                target: Some(|mut values, _| {
                    values.background = Color::TRANSPARENT;
                    values
                }),
                sibling_ordering: TransitionExitOrdering::Above,
                ..TransitionExit::default()
            },
            ..transition(TransitionProperties::BACKGROUND_COLOR)
        });
    let shown = Node::new()
        .layout(Layout {
            direction: Direction::Row,
            ..Layout::default()
        })
        .child(exiting)
        .child(Node::new().id("survivor").layout(Layout {
            sizing: Sizing::fixed(10.0, 10.0),
            ..Layout::default()
        }));
    let hidden = Node::new()
        .layout(Layout {
            direction: Direction::Row,
            ..Layout::default()
        })
        .child(Node::new().id("survivor").layout(Layout {
            sizing: Sizing::fixed(10.0, 10.0),
            ..Layout::default()
        }));
    let mut engine = engine();

    engine.layout(&shown, Size::new(100.0, 20.0), 0.0);
    let result = engine.layout(&hidden, Size::new(100.0, 20.0), 0.5);

    assert_eq!(
        result.element("exiting").unwrap().bounds,
        Rect::new(0.0, 0.0, 10.0, 10.0)
    );
}

#[test]
fn enter_first_frame_and_parent_trigger_modes() {
    let triggered = Transition {
        enter: TransitionEnter {
            initial: Some(offset_y),
            trigger: TransitionEnterTrigger::OnFirstParentFrame,
        },
        ..transition(TransitionProperties::Y)
    };
    let skipped = Transition {
        enter: TransitionEnter {
            initial: Some(offset_y),
            ..TransitionEnter::default()
        },
        ..transition(TransitionProperties::Y)
    };
    let mut engine = engine();

    let first = engine.layout(
        &Node::new()
            .id("parent")
            .child(box_node("triggered", 10.0, triggered))
            .child(box_node("skipped", 10.0, skipped)),
        Size::new(100.0, 100.0),
        0.5,
    );

    assert_eq!(first.element("triggered").unwrap().bounds.y, 20.0);
    assert_eq!(first.element("skipped").unwrap().bounds.y, 0.0);
}

#[test]
fn exit_preserves_subtree_final_frame_and_ordering() {
    let exit = |ordering| Transition {
        exit: TransitionExit {
            target: Some(offset_y),
            sibling_ordering: ordering,
            ..TransitionExit::default()
        },
        ..transition(TransitionProperties::Y)
    };
    let shown = Node::new()
        .id("root")
        .child(
            box_node("under", 10.0, exit(TransitionExitOrdering::Underneath))
                .background(Color::rgb(1.0, 1.0, 1.0)),
        )
        .child(
            box_node("natural", 10.0, exit(TransitionExitOrdering::Natural))
                .background(Color::rgb(2.0, 2.0, 2.0)),
        )
        .child(
            box_node("above", 10.0, exit(TransitionExitOrdering::Above))
                .background(Color::rgb(3.0, 3.0, 3.0)),
        );
    let hidden = Node::new().id("root").child(
        Node::new()
            .id("survivor")
            .background(Color::rgb(4.0, 4.0, 4.0)),
    );
    let mut engine = engine();

    engine.layout(&shown, Size::new(100.0, 100.0), 0.0);
    engine.layout(&hidden, Size::new(100.0, 100.0), 1.0);
    let final_frame = engine.layout(&hidden, Size::new(100.0, 100.0), 0.0);
    let gone = engine.layout(&hidden, Size::new(100.0, 100.0), 0.0);

    for id in ["under", "natural", "above"] {
        assert!(final_frame.element(id).is_some());
        assert!(gone.element(id).is_none());
    }
    let ids: Vec<_> = final_frame
        .commands
        .iter()
        .filter_map(|command| command.id.as_deref())
        .collect();
    assert!(ids.iter().position(|id| *id == "under") < ids.iter().position(|id| *id == "survivor"));
    assert!(ids.iter().position(|id| *id == "above") > ids.iter().position(|id| *id == "survivor"));
}

#[test]
fn interaction_validation_and_delta_time_rules() {
    for (interaction, expected) in [
        (TransitionInteraction::Disable, false),
        (TransitionInteraction::Allow, true),
    ] {
        let mut engine = engine();
        engine.input_mut().set_mouse_position(Point::new(0.0, 5.0));
        let old = Node::new().child(box_node(
            "button",
            10.0,
            Transition {
                interaction,
                ..transition(TransitionProperties::X)
            },
        ));
        let moved = Node::new()
            .layout(Layout {
                padding: Padding::new(20.0, 0.0, 0.0, 0.0),
                ..Layout::default()
            })
            .child(box_node(
                "button",
                10.0,
                Transition {
                    interaction,
                    ..transition(TransitionProperties::X)
                },
            ));
        engine.layout(&old, Size::new(100.0, 20.0), f32::NAN);
        let result = engine.layout(&moved, Size::new(100.0, 20.0), -1.0);
        assert_eq!(result.pointer_over("button"), expected);
    }

    let invalid = Node::new()
        .child(Node::new().transition(transition(TransitionProperties::WIDTH)))
        .child(
            Node::text("x", rlay::TextStyle::default())
                .id("text")
                .transition(transition(TransitionProperties::WIDTH)),
        )
        .child(Node::new().id("duplicate"))
        .child(Node::new().id("duplicate"));
    let errors = engine()
        .layout(&invalid, Size::new(100.0, 20.0), 0.0)
        .errors;

    assert!(errors.contains(&LayoutError::TransitionMissingId));
    assert!(errors.contains(&LayoutError::TextTransitionUnsupported));
    assert!(
        errors
            .iter()
            .any(|error| matches!(error, LayoutError::DuplicateElementId(_)))
    );
}

#[test]
fn reserved_public_ids_are_rejected_with_their_nodes() {
    let result = engine().layout(
        &Node::new()
            .child(Node::new().id("__rlay_panel"))
            .child(Node::new().id("valid")),
        Size::new(100.0, 20.0),
        0.0,
    );

    assert_eq!(
        result.errors,
        vec![LayoutError::ReservedElementId("__rlay_panel".into())]
    );
    assert!(result.element("__rlay_panel").is_none());
    assert!(result.element("valid").is_some());
}

#[test]
fn viewport_resize_and_zero_duration_snap() {
    let percent = Node::new()
        .id("box")
        .layout(Layout {
            sizing: Sizing {
                width: rlay::AxisSize::Percent(1.0),
                height: rlay::AxisSize::fixed(10.0),
            },
            ..Layout::default()
        })
        .transition(transition(TransitionProperties::WIDTH));
    let mut resize_engine = engine();
    resize_engine.layout(&percent, Size::new(100.0, 20.0), 0.0);
    let resized = resize_engine.layout(&percent, Size::new(200.0, 20.0), 0.5);
    assert_eq!(resized.element("box").unwrap().bounds.width, 200.0);

    let instant = Transition::ease_out(0.0, TransitionProperties::WIDTH);
    let mut engine = engine();
    engine.layout(
        &Node::new().child(box_node("box", 10.0, instant)),
        Size::new(100.0, 20.0),
        0.0,
    );
    let result = engine.layout(
        &Node::new().child(box_node("box", 20.0, instant)),
        Size::new(100.0, 20.0),
        0.0,
    );
    assert_eq!(result.element("box").unwrap().bounds.width, 20.0);
}

#[test]
fn parent_motion_and_scroll_do_not_trigger_child_position() {
    let tree = |padding, scroll| {
        Node::new()
            .layout(Layout {
                padding: Padding::new(padding, 0.0, 0.0, 0.0),
                ..Layout::default()
            })
            .child(
                Node::new()
                    .id("parent")
                    .scroll(false, scroll)
                    .layout(Layout {
                        direction: Direction::Column,
                        ..Layout::default()
                    })
                    .child(box_node(
                        "child",
                        10.0,
                        transition(TransitionProperties::POSITION),
                    )),
            )
    };
    let mut engine = engine();

    engine.layout(&tree(0.0, false), Size::new(100.0, 20.0), 0.0);
    let moved = engine.layout(&tree(20.0, false), Size::new(100.0, 20.0), 0.5);
    assert_eq!(moved.element("child").unwrap().bounds.x, 20.0);

    engine.set_query_scroll_offset(|id| {
        if id == "parent" {
            (0.0, 5.0).into()
        } else {
            rlay::Vector::ZERO
        }
    });
    let scrolled = engine.layout(&tree(20.0, true), Size::new(100.0, 20.0), 0.5);
    assert_eq!(scrolled.element("child").unwrap().bounds.y, -5.0);
}

#[test]
fn reparenting_animates_without_resizing_the_new_parent() {
    let tree = |right: bool, width| {
        let child = box_node(
            "child",
            width,
            transition(TransitionProperties::X | TransitionProperties::WIDTH),
        );
        let mut left = Node::new().id("left").layout(Layout {
            sizing: Sizing::fixed(50.0, 20.0),
            ..Layout::default()
        });
        let mut right_parent = Node::new().id("right").layout(Layout {
            sizing: Sizing::fixed(50.0, 20.0),
            ..Layout::default()
        });
        if right {
            right_parent.children.push(child);
        } else {
            left.children.push(child);
        }
        Node::new()
            .layout(Layout {
                direction: Direction::Row,
                ..Layout::default()
            })
            .child(left)
            .child(right_parent)
    };
    let mut engine = engine();

    engine.layout(&tree(false, 10.0), Size::new(100.0, 20.0), 0.0);
    engine.layout(&tree(true, 20.0), Size::new(100.0, 20.0), 0.5);
    let animated = engine.layout(&tree(true, 20.0), Size::new(100.0, 20.0), 0.0);

    assert!(animated.element("child").unwrap().bounds.x < 50.0);
    assert_eq!(animated.element("right").unwrap().bounds.width, 50.0);
}

#[test]
fn exit_floats_without_parent_and_omits_reparented_children() {
    let exit = Transition {
        exit: TransitionExit {
            target: Some(offset_y),
            trigger: rlay::TransitionExitTrigger::WhenParentExits,
            ..TransitionExit::default()
        },
        ..transition(TransitionProperties::Y)
    };
    let shown = Node::new().child(
        Node::new().id("parent").child(
            box_node("exiting", 10.0, exit)
                .background(Color::rgb(1.0, 1.0, 1.0))
                .child(
                    Node::new()
                        .id("moved")
                        .background(Color::rgb(2.0, 2.0, 2.0)),
                ),
        ),
    );
    let hidden = Node::new().child(
        Node::new().id("new-parent").child(
            Node::new()
                .id("moved")
                .background(Color::rgb(2.0, 2.0, 2.0)),
        ),
    );
    let mut engine = engine();

    engine.layout(&shown, Size::new(100.0, 100.0), 0.0);
    let result = engine.layout(&hidden, Size::new(100.0, 100.0), 0.5);

    assert!(result.element("exiting").is_some());
    assert_eq!(
        result
            .commands
            .iter()
            .filter(|command| command.id.as_deref() == Some("moved"))
            .count(),
        1
    );
}

#[test]
fn nested_exit_uses_the_outer_snapshot_once() {
    let exit = Transition {
        exit: TransitionExit {
            target: Some(offset_y),
            trigger: rlay::TransitionExitTrigger::WhenParentExits,
            ..TransitionExit::default()
        },
        ..transition(TransitionProperties::Y)
    };
    let shown = Node::new().child(
        box_node("outer", 10.0, exit)
            .background(Color::rgb(1.0, 1.0, 1.0))
            .child(box_node("inner", 10.0, exit).background(Color::rgb(2.0, 2.0, 2.0))),
    );
    let mut engine = engine();

    engine.layout(&shown, Size::new(100.0, 100.0), 0.0);
    let result = engine.layout(&Node::new(), Size::new(100.0, 100.0), 0.5);

    assert_eq!(
        result
            .commands
            .iter()
            .filter(|command| command.id.as_deref() == Some("inner"))
            .count(),
        1
    );
}
