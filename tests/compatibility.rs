use rlay::{
    Color, CommandKind, Direction, Engine, Floating, Layout, Node, Padding, Rect, RenderCommand,
    Size, Sizing, TextStyle, Vector,
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
fn clay_like_row_fixture_matches_expected_commands() {
    let root_color = Color::rgba(10.0, 10.0, 10.0, 255.0);
    let text_style = TextStyle {
        font_size: 8.0,
        ..TextStyle::default()
    };
    let root = Node::new()
        .id("root")
        .background(root_color)
        .layout(Layout {
            direction: Direction::Row,
            padding: Padding::all(2.0),
            gap: 4.0,
            ..Layout::default()
        })
        .child(Node::text("A", text_style.clone()).id("label"))
        .child(
            Node::new()
                .id("box")
                .background(Color::rgba(1.0, 2.0, 3.0, 255.0))
                .layout(Layout {
                    sizing: Sizing::fixed(10.0, 8.0),
                    ..Layout::default()
                }),
        );

    let commands = engine().layout(&root, Size::new(40.0, 16.0), 0.0).commands;

    assert_eq!(
        commands,
        vec![
            RenderCommand {
                id: Some("root".into()),
                bounds: Rect::new(0.0, 0.0, 40.0, 16.0),
                kind: CommandKind::Rectangle {
                    color: root_color,
                    radius: Default::default(),
                },
            },
            RenderCommand {
                id: Some("label".into()),
                bounds: Rect::new(2.0, 2.0, 8.0, 8.0),
                kind: CommandKind::Text {
                    text: "A".into(),
                    style: text_style,
                },
            },
            RenderCommand {
                id: Some("box".into()),
                bounds: Rect::new(14.0, 2.0, 10.0, 8.0),
                kind: CommandKind::Rectangle {
                    color: Color::rgba(1.0, 2.0, 3.0, 255.0),
                    radius: Default::default(),
                },
            },
        ]
    );
}

#[test]
fn clay_like_floating_fixture_matches_expected_bounds() {
    let root = Node::new().child(
        Node::new()
            .id("float")
            .floating(Floating {
                offset: Vector::new(5.0, 6.0),
                ..Floating::parent()
            })
            .layout(Layout {
                sizing: Sizing::fixed(10.0, 10.0),
                ..Layout::default()
            }),
    );

    let result = engine().layout(&root, Size::new(100.0, 80.0), 0.0);

    assert_eq!(
        result.element("float").unwrap().bounds,
        Rect::new(5.0, 6.0, 10.0, 10.0)
    );
}

#[test]
fn clay_like_scroll_fixture_matches_expected_offset_bounds() {
    let root = Node::new()
        .id("scroll")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().id("row").layout(Layout {
            sizing: Sizing::fixed(20.0, 40.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    engine.set_scroll_offset("scroll", Vector::new(0.0, 12.0));

    let result = engine.layout(&root, Size::new(20.0, 20.0), 0.0);

    assert_eq!(
        result.element("row").unwrap().bounds,
        Rect::new(0.0, -12.0, 20.0, 40.0)
    );
    assert_eq!(
        result.scroll_container("scroll").unwrap().content_size,
        Size::new(20.0, 40.0)
    );
}
