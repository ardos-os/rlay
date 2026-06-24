use rlay::{
    Anchor, AttachTo, Engine, Floating, Layout, Node, Point, PointerCapture, Rect, Size, Sizing,
    Vector,
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
fn floating_child_attaches_to_parent_without_taking_flow_space() {
    let floating = Floating {
        target_anchor: Anchor::BOTTOM_RIGHT,
        element_anchor: Anchor::BOTTOM_RIGHT,
        ..Floating::parent()
    };
    let root = Node::new()
        .id("root")
        .child(Node::new().id("flow").layout(Layout {
            sizing: Sizing::fixed(20.0, 20.0),
            ..Layout::default()
        }))
        .child(Node::new().id("float").floating(floating).layout(Layout {
            sizing: Sizing::fixed(10.0, 10.0),
            ..Layout::default()
        }));

    let result = engine().layout(&root, Size::new(100.0, 80.0), 0.0);

    assert_eq!(
        result.element("flow").unwrap().bounds,
        Rect::new(0.0, 0.0, 20.0, 20.0)
    );
    assert_eq!(
        result.element("float").unwrap().bounds,
        Rect::new(90.0, 70.0, 10.0, 10.0)
    );
}

#[test]
fn floating_child_can_attach_to_existing_element() {
    let floating = Floating {
        attach_to: AttachTo::Element("button".into()),
        target_anchor: Anchor::BOTTOM_RIGHT,
        offset: Vector::new(5.0, 0.0),
        ..Floating::parent()
    };
    let root = Node::new()
        .child(Node::new().id("button").layout(Layout {
            sizing: Sizing::fixed(40.0, 20.0),
            ..Layout::default()
        }))
        .child(Node::new().id("menu").floating(floating).layout(Layout {
            sizing: Sizing::fixed(30.0, 10.0),
            ..Layout::default()
        }));

    let result = engine().layout(&root, Size::new(100.0, 80.0), 0.0);

    assert_eq!(
        result.element("menu").unwrap().bounds,
        Rect::new(45.0, 20.0, 30.0, 10.0)
    );
}

#[test]
fn pass_through_floating_does_not_capture_hit_test() {
    let floating = Floating {
        pointer_capture: PointerCapture::PassThrough,
        ..Floating::parent()
    };
    let root = Node::new()
        .id("root")
        .child(Node::new().id("float").floating(floating).layout(Layout {
            sizing: Sizing::fixed(100.0, 80.0),
            ..Layout::default()
        }));
    let mut engine = engine();
    let result = engine.layout(&root, Size::new(100.0, 80.0), 0.0);

    assert_eq!(
        Engine::hit_test(&result, Point::new(10.0, 10.0)),
        Some("root")
    );
}

#[test]
fn floating_can_inherit_parent_clip() {
    let floating = Floating {
        clip_to_parent: true,
        offset: Vector::new(30.0, 0.0),
        ..Floating::parent()
    };
    let root = Node::new().id("root").clip(true, false).child(
        Node::new().id("float").floating(floating).layout(Layout {
            sizing: Sizing::fixed(40.0, 20.0),
            ..Layout::default()
        }),
    );
    let mut engine = engine();
    let result = engine.layout(&root, Size::new(50.0, 20.0), 0.0);

    assert_eq!(
        Engine::hit_test(&result, Point::new(45.0, 10.0)),
        Some("float")
    );
    assert_eq!(Engine::hit_test(&result, Point::new(60.0, 10.0)), None);
}
