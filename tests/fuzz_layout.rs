use rlay::{
    AlignX, AlignY, AxisSize, Color, Direction, Engine, Layout, Node, Padding, Point, Size, Sizing,
    TextStyle,
};

#[derive(Clone)]
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
        (self.0 >> 32) as u32
    }

    fn bool(&mut self) -> bool {
        self.next() & 1 == 0
    }

    fn range(&mut self, min: f32, max: f32) -> f32 {
        min + (self.next() as f32 / u32::MAX as f32) * (max - min)
    }

    fn usize(&mut self, max: usize) -> usize {
        (self.next() as usize) % max
    }
}

fn engine() -> Engine {
    Engine::new(|text, style| {
        Size::new(
            text.chars().count() as f32 * style.font_size,
            style.font_size,
        )
    })
}

fn random_axis(rng: &mut Rng) -> AxisSize {
    match rng.usize(4) {
        0 => AxisSize::fixed(rng.range(0.0, 80.0)),
        1 => AxisSize::Percent(rng.range(-0.5, 1.5)),
        2 => AxisSize::Grow {
            min: 0.0,
            max: rng.range(1.0, 120.0),
        },
        _ => AxisSize::FIT,
    }
}

fn random_node(rng: &mut Rng, depth: usize, index: usize) -> Node {
    let layout = Layout {
        sizing: Sizing {
            width: random_axis(rng),
            height: random_axis(rng),
        },
        padding: Padding::all(rng.range(0.0, 8.0)),
        gap: rng.range(0.0, 8.0),
        direction: if rng.bool() {
            Direction::Row
        } else {
            Direction::Column
        },
        align_x: [AlignX::Left, AlignX::Center, AlignX::Right][rng.usize(3)],
        align_y: [AlignY::Top, AlignY::Center, AlignY::Bottom][rng.usize(3)],
    };
    let mut node = if depth == 0 || rng.bool() {
        if rng.bool() {
            Node::text("abc def", TextStyle::default())
        } else {
            Node::new().background(Color::rgba(1.0, 2.0, 3.0, 255.0))
        }
    } else {
        let mut node = Node::new();
        for child in 0..rng.usize(4) {
            node = node.child(random_node(rng, depth - 1, child));
        }
        node
    };
    node = node
        .id(format!("n{depth}_{index}_{}", rng.next()))
        .layout(layout);
    if rng.bool() {
        node = node.clip(rng.bool(), rng.bool());
    }
    if rng.usize(8) == 0 {
        node = node.scroll(rng.bool(), rng.bool());
    }
    node
}

#[test]
fn fuzz_layout_never_emits_non_finite_bounds_or_panics() {
    for seed in 0..200 {
        let mut rng = Rng::new(seed);
        let root = random_node(&mut rng, 4, 0);
        let mut engine = engine();
        engine
            .input_mut()
            .set_mouse_position(Point::new(rng.range(-50.0, 250.0), rng.range(-50.0, 250.0)));
        let result = engine.layout(
            &root,
            Size::new(rng.range(1.0, 240.0), rng.range(1.0, 240.0)),
        );

        for command in &result.commands {
            assert!(command.bounds.x.is_finite());
            assert!(command.bounds.y.is_finite());
            assert!(command.bounds.width.is_finite());
            assert!(command.bounds.height.is_finite());
            assert!(command.bounds.width >= 0.0);
            assert!(command.bounds.height >= 0.0);
        }
        for element in result.elements.values() {
            assert!(element.bounds.width >= 0.0);
            assert!(element.bounds.height >= 0.0);
        }
    }
}
