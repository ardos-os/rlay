# rlay

`rlay` is an independent, safe Rust rewrite of the
[Clay](https://github.com/nicbarker/clay) layout engine.

It does not compile, link to, wrap, or otherwise depend on the original C
implementation. The engine is implemented entirely in Rust while preserving
Clay-compatible layout behavior.


Rlay aims to provide a faithful reproduction of the original Clay's layout algorithms.
It takes a tree of nodes and produces element bounds, ordered render commands,
pointer hits, and persistent scroll state. Window creation, font loading, text
measurement, and drawing remain the responsibility of the host application.

On top of that `rlay` adds Rust-native APIs, touchscreen multi touch support and touchscreen scrolling with momentum and overscroll, allowing you to easily have professional looking and natural scrolling that feels just right when scrolling with your finger without you having to write a single line of code.

## Status

`rlay` is currently at version `0.1.0`. The API is usable but not yet stable.

Implemented features include:

- Row and column layout
- `FIT`, `GROW`, `FIXED`, and `PERCENT` sizing
- Padding, gaps, alignment, borders, and aspect ratios
- Word, newline, and unwrapped text layout
- Rectangle, border, text, image, custom, clip, and overlay commands
- Mouse, multitouch, pointer capture, hit testing, and pinch gestures
- Touch, wheel, and touchpad scrolling with momentum and overscroll
- Floating elements attached to a parent, root, or element
- Direct retained trees and an immediate-mode frame builder

## Downsides

The main downsides of `rlay` compared to clay is that it doesn't support `#[no_std]`, embedded environments and it doesn't use an arena allocator.

Adding it from the start would add friction to the development of the library and I wouldn't really use `#[no_std]` support for my use case,
so I prioritized it out. This may change in the future.


## Usage

Add `rlay` as a git dependency:

```toml
[dependencies]
rlay = { git = "https://github.com/ardos-os/rlay" }
```

Create an engine with the text measurement function used by your renderer:

```rust
use rlay::{
    AxisSize, Color, Direction, Engine, Layout, Node, Padding, Size, Sizing,
    TextStyle,
};

let mut engine = Engine::new(|text, style| {
    // Replace this with measurement from your font backend.
    Size::new(
        text.chars().count() as f32 * style.font_size * 0.5,
        style.font_size,
    )
});

let root = Node::new()
    .layout(Layout {
        direction: Direction::Row,
        padding: Padding::all(12.0),
        gap: 8.0,
        ..Layout::default()
    })
    .child(
        Node::text("Text wraps when the row is compressed.", TextStyle::default())
            .id("label"),
    )
    .child(
        Node::new()
            .id("panel")
            .background(Color::rgba(30.0, 90.0, 180.0, 255.0))
            .layout(Layout {
                sizing: Sizing {
                    width: AxisSize::GROW,
                    height: AxisSize::fixed(40.0),
                },
                ..Layout::default()
            }),
    );

let result = engine.layout(&root, Size::new(320.0, 200.0), 0.0);

for command in &result.commands {
    // Translate each command into calls to your renderer.
    println!("{command:?}");
}

let panel_bounds = result.element("panel").unwrap().bounds;
```

## Frame Flow

A typical frame consists of:

1. Updating mouse, touch, or wheel state through `engine.input_mut()`.
2. Building the `Node` tree.
3. Calling `engine.layout(&root, viewport_size, delta_time_seconds)`.
4. Drawing `result.commands` in order.
5. Reading element, pointer, and scroll queries from the result.

For event-driven applications, `Engine::apply_input_scroll` can update scroll
state from an existing layout before the next layout pass.

If you have multiple outputs (windows, monitors, etc...), you must create and manage a different Engine per root.

## Transitions

Attach transitions to nodes with stable ids:

```rust
use rlay::{Node, Transition, TransitionProperties};

let panel = Node::new()
    .id("panel")
    .transition(Transition::ease_out(
        0.2,
        TransitionProperties::BOUNDS | TransitionProperties::BACKGROUND_COLOR,
    ));
```

Pass frame time in seconds to `Engine::layout` or `Frame::end`. Negative and
non-finite values count as zero. Invalid transition declarations are reported
through `LayoutResult::errors`.

## Rendering

`rlay` does not depend on a graphics backend. Match on `CommandKind` and render
the supplied bounds and payload using Skia, wgpu, OpenGL, a platform canvas, or
another renderer.

Text measurement must use the same fonts and logical-pixel scale as rendering.
Call `Engine::reset_measure_text_cache` after changing fonts or DPI scale.

## Development

Run the interactive transitions example:

```sh
cargo run --example transitions
```

It mirrors Clay's `raylib-transitions` demo: randomise, recolor, add and remove
animated boxes in a responsive 5×6 grid.

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo bench --bench layout
```
