use std::{cell::Cell, rc::Rc};

use rlay::{
    AxisSize, CommandKind, Direction, ElementId, Engine, Layout, LayoutError, Node, Rect,
    RenderCommand, Size, Sizing, TextAlign, TextStyle, TextWrap,
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
fn text_measurement_cache_can_be_reset() {
    let calls = Rc::new(Cell::new(0));
    let calls_for_measure = Rc::clone(&calls);
    let mut engine = Engine::new(move |text, style| {
        calls_for_measure.set(calls_for_measure.get() + 1);
        Size::new(
            text.chars().count() as f32 * style.font_size,
            style.font_size,
        )
    });
    let style = TextStyle::default();

    assert_eq!(engine.measure_text("abc", &style), Size::new(48.0, 16.0));
    assert_eq!(engine.measure_text("abc", &style), Size::new(48.0, 16.0));
    assert_eq!(engine.measure_text_cache_len(), 1);
    assert_eq!(calls.get(), 1);

    engine.reset_measure_text_cache();
    assert_eq!(engine.measure_text_cache_len(), 0);
    assert_eq!(engine.measure_text("abc", &style), Size::new(48.0, 16.0));
    assert_eq!(calls.get(), 2);
}

#[test]
fn immediate_mode_frame_builds_nested_layout() {
    let mut engine = engine();
    let mut frame = engine.begin(Size::new(100.0, 40.0));

    frame.open(Node::new().id("row").layout(Layout {
        sizing: Sizing::fixed(100.0, 40.0),
        ..Layout::default()
    }));
    frame.child(Node::new().id("child").layout(Layout {
        sizing: Sizing::fixed(20.0, 10.0),
        ..Layout::default()
    }));
    frame.close().unwrap();
    let result = frame.end(0.0).unwrap();

    assert_eq!(
        result.element("child").unwrap().bounds,
        Rect::new(0.0, 0.0, 20.0, 10.0)
    );
}

#[test]
fn immediate_mode_reports_unbalanced_frames() {
    let mut engine = engine();
    let mut frame = engine.begin(Size::new(100.0, 40.0));

    frame.open(Node::new().id("open"));

    assert_eq!(frame.end(0.0).unwrap_err(), LayoutError::UnclosedElements);
}

#[test]
fn image_node_emits_image_command() {
    let root = Node::image(42).id("image").layout(Layout {
        sizing: Sizing::fixed(30.0, 20.0),
        ..Layout::default()
    });

    let result = engine().layout(&root, Size::new(30.0, 20.0), 0.0);

    assert_eq!(
        result.commands,
        vec![RenderCommand {
            id: Some("image".into()),
            bounds: Rect::new(0.0, 0.0, 30.0, 20.0),
            kind: CommandKind::Image(42),
        }]
    );
}

#[test]
fn aspect_ratio_derives_fit_axis_from_fixed_axis() {
    let root = Node::new().child(Node::new().id("poster").aspect_ratio(2.0).layout(Layout {
        sizing: Sizing {
            width: AxisSize::fixed(80.0),
            height: AxisSize::FIT,
        },
        ..Layout::default()
    }));

    let result = engine().layout(&root, Size::new(100.0, 100.0), 0.0);

    assert_eq!(
        result.element("poster").unwrap().bounds,
        Rect::new(0.0, 0.0, 80.0, 40.0)
    );
}

#[test]
fn text_wraps_words_and_aligns_lines() {
    let style = TextStyle {
        font_size: 10.0,
        line_height: 12.0,
        wrap: TextWrap::Words,
        align: TextAlign::Right,
        ..TextStyle::default()
    };
    let root = Node::text("aa bb cc", style.clone())
        .id("text")
        .layout(Layout {
            sizing: Sizing {
                width: AxisSize::fixed(50.0),
                height: AxisSize::FIT,
            },
            ..Layout::default()
        });

    let result = engine().layout(&root, Size::new(50.0, 100.0), 0.0);

    assert_eq!(
        result.commands,
        vec![
            RenderCommand {
                id: Some("text".into()),
                bounds: Rect::new(0.0, 0.0, 50.0, 12.0),
                kind: CommandKind::Text {
                    text: "aa bb".into(),
                    style: style.clone(),
                },
            },
            RenderCommand {
                id: Some("text".into()),
                bounds: Rect::new(30.0, 12.0, 20.0, 12.0),
                kind: CommandKind::Text {
                    text: "cc".into(),
                    style,
                },
            },
        ]
    );
}

#[test]
fn row_compresses_fit_content_so_text_wraps() {
    let style = TextStyle {
        font_size: 10.0,
        line_height: 10.0,
        ..TextStyle::default()
    };
    let root = Node::new()
        .layout(Layout {
            direction: Direction::Row,
            ..Layout::default()
        })
        .child(Node::text("aaaa bbbb cccc", style.clone()).id("fit"))
        .child(Node::text("dddd", style).id("grow").layout(Layout {
            sizing: Sizing {
                width: AxisSize::GROW,
                height: AxisSize::FIT,
            },
            ..Layout::default()
        }));

    let result = engine().layout(&root, Size::new(100.0, 100.0), 0.0);

    assert_eq!(result.element("fit").unwrap().bounds.width, 60.0);
    assert_eq!(result.element("fit").unwrap().bounds.height, 30.0);
    assert_eq!(result.element("grow").unwrap().bounds.width, 40.0);
}

#[test]
fn nested_fit_container_propagates_wrapped_text_height() {
    let style = TextStyle {
        font_size: 10.0,
        line_height: 10.0,
        ..TextStyle::default()
    };
    let root = Node::new()
        .layout(Layout {
            direction: Direction::Row,
            ..Layout::default()
        })
        .child(
            Node::new()
                .id("fit")
                .child(Node::text("aaaa bbbb cccc", style.clone())),
        )
        .child(Node::text("dddd", style).id("grow").layout(Layout {
            sizing: Sizing {
                width: AxisSize::GROW,
                height: AxisSize::FIT,
            },
            ..Layout::default()
        }));

    let result = engine().layout(&root, Size::new(100.0, 100.0), 0.0);

    assert_eq!(
        result.element("fit").unwrap().bounds,
        Rect::new(0.0, 0.0, 60.0, 30.0)
    );
    assert_eq!(result.element("grow").unwrap().bounds.width, 40.0);
}

#[test]
fn word_wrap_preserves_the_largest_word_minimum() {
    let style = TextStyle {
        font_size: 10.0,
        line_height: 10.0,
        ..TextStyle::default()
    };
    let root = Node::new().child(Node::text("abcdefghij x", style).id("text"));

    let result = engine().layout(&root, Size::new(50.0, 100.0), 0.0);

    assert_eq!(
        result.element("text").unwrap().bounds,
        Rect::new(0.0, 0.0, 100.0, 20.0)
    );
    assert_eq!(result.commands[0].bounds.width, 100.0);
}

#[test]
fn row_percent_sizing_excludes_child_gaps() {
    let root = Node::new()
        .layout(Layout {
            direction: Direction::Row,
            gap: 10.0,
            ..Layout::default()
        })
        .child(Node::new().id("percent").layout(Layout {
            sizing: Sizing {
                width: AxisSize::Percent(0.5),
                height: AxisSize::fixed(10.0),
            },
            ..Layout::default()
        }))
        .child(Node::new().id("grow").layout(Layout {
            sizing: Sizing {
                width: AxisSize::GROW,
                height: AxisSize::fixed(10.0),
            },
            ..Layout::default()
        }));

    let result = engine().layout(&root, Size::new(100.0, 20.0), 0.0);

    assert_eq!(result.element("percent").unwrap().bounds.width, 45.0);
    assert_eq!(
        result.element("grow").unwrap().bounds,
        Rect::new(55.0, 0.0, 45.0, 10.0)
    );
}

#[test]
fn stable_ids_are_repeatable_and_can_be_attached_to_nodes() {
    let id = ElementId::indexed("item", 2);
    let same = ElementId::indexed("item", 2);
    let other = ElementId::indexed("item", 3);
    let root = Node::new().element_id(id.clone());

    let result = engine().layout(&root, Size::new(10.0, 10.0), 0.0);

    assert_eq!(id.hash, same.hash);
    assert_ne!(id.hash, other.hash);
    assert_eq!(
        result.element("item").unwrap().element_id.as_ref().unwrap(),
        &id
    );
}
