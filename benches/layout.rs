use criterion::{Criterion, criterion_group, criterion_main};
use rlay::{Direction, Engine, Layout, Node, Size, Sizing, TextStyle};
use std::hint::black_box;

fn engine() -> Engine {
    Engine::new(|text, style| {
        Size::new(
            text.chars().count() as f32 * style.font_size,
            style.font_size,
        )
    })
}

fn large_tree(count: usize) -> Node {
    let mut root = Node::new().id("root").layout(Layout {
        direction: Direction::Column,
        ..Layout::default()
    });
    for index in 0..count {
        root = root.child(
            Node::new()
                .id(format!("row-{index}"))
                .layout(Layout {
                    sizing: Sizing::fixed(400.0, 18.0),
                    ..Layout::default()
                })
                .child(Node::text(format!("row {index}"), TextStyle::default())),
        );
    }
    root
}

fn bench_large_list(c: &mut Criterion) {
    let root = large_tree(500);
    c.bench_function("rlay/large_list_500", |b| {
        b.iter_batched(
            engine,
            |mut engine| {
                let result =
                    engine.layout(black_box(&root), black_box(Size::new(400.0, 800.0)), 0.0);
                black_box(result.commands.len())
            },
            criterion::BatchSize::SmallInput,
        )
    });

    c.bench_function("rlay/large_list_500_persistent_engine", |b| {
        let mut engine = engine();
        b.iter(|| {
            let result = engine.layout(black_box(&root), black_box(Size::new(400.0, 800.0)), 0.0);
            black_box(result.commands.len())
        });
    });

    c.bench_function("rlay/immediate_large_list_500", |b| {
        let mut engine = engine();
        b.iter(|| black_box(immediate_large_list(&mut engine, 500)));
    });
}

fn immediate_large_list(engine: &mut Engine, count: usize) -> usize {
    let mut frame = engine.begin(Size::new(400.0, 800.0));
    frame.open(Node::new().id("root").layout(Layout {
        direction: Direction::Column,
        ..Layout::default()
    }));
    for index in 0..count {
        frame.open(Node::new().id(format!("row-{index}")).layout(Layout {
            sizing: Sizing::fixed(400.0, 18.0),
            ..Layout::default()
        }));
        frame.text(format!("row {index}"), TextStyle::default());
        frame.close().unwrap();
    }
    frame.close().unwrap();
    let result = frame.end(0.0).unwrap();
    result.commands.len()
}

criterion_group!(benches, bench_large_list);
criterion_main!(benches);
