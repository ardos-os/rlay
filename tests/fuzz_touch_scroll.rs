use rlay::{Direction, Engine, Layout, Node, Point, Size, Sizing, Vector};

#[derive(Clone)]
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1103515245).wrapping_add(12345);
        (self.0 >> 16) as u32
    }

    fn range(&mut self, min: f32, max: f32) -> f32 {
        min + (self.next() as f32 / u32::MAX as f32) * (max - min)
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

#[test]
fn fuzz_touch_scroll_offsets_stay_bounded_and_do_not_scale_by_finger_count() {
    let root = Node::new()
        .id("list")
        .scroll(false, true)
        .layout(Layout {
            direction: Direction::Column,
            ..Layout::default()
        })
        .child(Node::new().layout(Layout {
            sizing: Sizing::fixed(100.0, 300.0),
            ..Layout::default()
        }));

    for seed in 0..100 {
        let mut rng = Rng(seed);
        let dy = rng.range(-30.0, 30.0);
        let mut one = engine();
        one.input_mut().set_touch(1, Point::new(10.0, 50.0), true);
        one.layout(&root, Size::new(100.0, 100.0));
        one.input_mut()
            .set_touch(1, Point::new(10.0, 50.0 + dy), true);
        one.layout(&root, Size::new(100.0, 100.0));
        let one_offset = one
            .layout(&root, Size::new(100.0, 100.0))
            .scroll_container("list")
            .unwrap()
            .offset;

        let mut two = engine();
        two.input_mut().set_touch(1, Point::new(10.0, 50.0), true);
        two.input_mut().set_touch(2, Point::new(20.0, 50.0), true);
        two.layout(&root, Size::new(100.0, 100.0));
        two.input_mut()
            .set_touch(1, Point::new(10.0, 50.0 + dy), true);
        two.input_mut()
            .set_touch(2, Point::new(20.0, 50.0 + dy), true);
        two.layout(&root, Size::new(100.0, 100.0));
        let two_offset = two
            .layout(&root, Size::new(100.0, 100.0))
            .scroll_container("list")
            .unwrap()
            .offset;

        assert_eq!(one_offset, two_offset);
        assert!(one_offset.y > -30.0 && one_offset.y < 230.0);
        assert_eq!(one_offset.x, Vector::ZERO.x);
    }
}
