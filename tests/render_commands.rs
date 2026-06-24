use rlay::{
    AlignY, AxisSize, Color, CommandKind, Direction, Engine, Layout, Node, Padding, Point, Radius,
    Rect, RenderCommand, Size, Sizing, TextStyle,
};

fn engine() -> Engine {
    Engine::new(|text, style| {
        Size::new(
            text.chars().count() as f32 * style.font_size / 2.0,
            style.line_height.max(style.font_size),
        )
    })
}

#[test]
fn emits_commands_for_nested_layout_in_draw_order() {
    let panel_color = Color::rgba(20.0, 20.0, 20.0, 255.0);
    let text_color = Color::rgba(240.0, 240.0, 240.0, 255.0);
    let text_style = TextStyle {
        color: text_color,
        font_size: 12.0,
        line_height: 18.0,
        ..TextStyle::default()
    };

    let root = Node::new()
        .id("root")
        .layout(Layout {
            padding: Padding::all(10.0),
            direction: Direction::Row,
            align_y: AlignY::Center,
            gap: 8.0,
            ..Layout::default()
        })
        .background(panel_color)
        .radius(Radius::all(6.0))
        .child(Node::text("Play", text_style.clone()).id("label"))
        .child(Node::custom(7).id("art").layout(Layout {
            sizing: Sizing {
                width: AxisSize::GROW,
                height: AxisSize::fixed(40.0),
            },
            ..Layout::default()
        }));

    let result = engine().layout(&root, Size::new(200.0, 80.0), 0.0);

    assert_eq!(
        result.elements["root"].bounds,
        Rect::new(0.0, 0.0, 200.0, 80.0)
    );
    assert_eq!(
        result.elements["label"].bounds,
        Rect::new(10.0, 31.0, 24.0, 18.0)
    );
    assert_eq!(
        result.elements["art"].bounds,
        Rect::new(42.0, 20.0, 148.0, 40.0)
    );

    assert_eq!(
        result.commands,
        vec![
            RenderCommand {
                id: Some("root".into()),
                bounds: Rect::new(0.0, 0.0, 200.0, 80.0),
                kind: CommandKind::Rectangle {
                    color: panel_color,
                    radius: Radius::all(6.0),
                },
            },
            RenderCommand {
                id: Some("label".into()),
                bounds: Rect::new(10.0, 31.0, 24.0, 18.0),
                kind: CommandKind::Text {
                    text: "Play".into(),
                    style: text_style,
                },
            },
            RenderCommand {
                id: Some("art".into()),
                bounds: Rect::new(42.0, 20.0, 148.0, 40.0),
                kind: CommandKind::Custom(7, Radius::default()),
            },
        ]
    );
}

#[test]
fn emits_clip_start_and_end_around_children() {
    let mut root = Node::new()
        .id("clipper")
        .layout(Layout {
            padding: Padding::all(4.0),
            ..Layout::default()
        })
        .child(Node::text("wide", TextStyle::default()).id("text"));
    root.clip_x = true;

    let result = engine().layout(&root, Size::new(40.0, 24.0), 0.0);

    assert!(matches!(
        result.commands[0],
        RenderCommand {
            id: Some(_),
            bounds: Rect {
                x: 0.0,
                y: 0.0,
                width: 40.0,
                height: 24.0,
            },
            kind: CommandKind::ClipStart { x: true, y: false },
        }
    ));
    assert!(matches!(
        result.commands.last().unwrap().kind,
        CommandKind::ClipEnd
    ));
}

#[test]
fn hit_test_respects_clipped_bounds() {
    let mut root = Node::new()
        .id("clipper")
        .child(Node::new().id("child").layout(Layout {
            sizing: Sizing::fixed(80.0, 20.0),
            ..Layout::default()
        }));
    root.clip_x = true;

    let mut engine = engine();
    let result = engine.layout(&root, Size::new(40.0, 20.0), 0.0);

    assert_eq!(
        Engine::hit_test(&result, Point::new(20.0, 10.0)),
        Some("child")
    );
    assert_eq!(Engine::hit_test(&result, Point::new(60.0, 10.0)), None);
}

#[test]
fn horizontal_clip_does_not_clip_vertical_hit_area() {
    let mut root = Node::new()
        .id("clipper")
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().id("child").layout(Layout {
            sizing: Sizing::fixed(20.0, 80.0),
            ..Layout::default()
        }));
    root.clip_x = true;

    let mut engine = engine();
    let result = engine.layout(&root, Size::new(20.0, 40.0), 0.0);

    assert_eq!(
        Engine::hit_test(&result, Point::new(10.0, 60.0)),
        Some("child")
    );
}
