use macroquad::prelude as mq;
use rlay::{
    AlignX, AlignY, AxisSize, Border, Color, Direction, Engine, Layout, Node, Padding, Point,
    Radius, Size, Sizing, TextStyle, Transition, TransitionEnter, TransitionExit,
    TransitionProperties, TransitionValues,
};

#[path = "../macroquad_renderer.rs"]
mod macroquad_renderer;

const PURPLE: Color = Color::rgba(174.0, 143.0, 204.0, 255.0);
const DARK_PURPLE: Color = Color::rgba(154.0, 123.0, 184.0, 255.0);
const WHITE: Color = Color::rgba(255.0, 255.0, 255.0, 255.0);

#[derive(Clone)]
struct BoxItem {
    id: usize,
    color: Color,
}

struct App {
    boxes: Vec<BoxItem>,
    next_id: usize,
    rng: u64,
}

impl App {
    fn new() -> Self {
        Self {
            boxes: (0..30)
                .map(|id| BoxItem {
                    id,
                    color: pink(id),
                })
                .collect(),
            next_id: 30,
            rng: 0x4d59_5df4_d0f3_3173,
        }
    }

    fn shuffle(&mut self) {
        for index in (1..self.boxes.len()).rev() {
            self.rng ^= self.rng << 13;
            self.rng ^= self.rng >> 7;
            self.rng ^= self.rng << 17;
            let other = self.rng as usize % (index + 1);
            self.boxes.swap(index, other);
        }
    }

    fn add(&mut self) {
        let id = self.next_id;
        self.next_id += 1;
        let index = self.rng as usize % (self.boxes.len() + 1);
        self.boxes.insert(
            index,
            BoxItem {
                id,
                color: pink(id),
            },
        );
    }

    fn click(&mut self, id: &str) {
        match id {
            "shuffle" => self.shuffle(),
            "blue" => {
                for item in &mut self.boxes {
                    item.color = blue(item.id);
                }
            }
            "pink" => {
                for item in &mut self.boxes {
                    item.color = pink(item.id);
                }
            }
            "add" => self.add(),
            _ => {
                if let Some(id) = id.strip_prefix("box-").and_then(|id| id.parse().ok()) {
                    self.boxes.retain(|item| item.id != id);
                }
            }
        }
    }

    fn tree(&self, hovered: Option<&str>, pressed: bool) -> Node {
        let mut root = Node::new()
            .background(WHITE)
            .layout(Layout {
                direction: Direction::Column,
                padding: Padding::all(16.0),
                gap: 12.0,
                ..Layout::default()
            })
            .child(toolbar(hovered));

        for row in 0..5 {
            let mut row_node = Node::new().id(format!("row-{row}")).layout(Layout {
                sizing: Sizing {
                    width: AxisSize::GROW,
                    height: AxisSize::GROW,
                },
                gap: 12.0,
                ..Layout::default()
            });

            for item in self.boxes.iter().skip(row as usize * 6).take(6) {
                let id = format!("box-{}", item.id);
                let is_hovered = hovered == Some(id.as_str());
                let darker = Color::rgba(
                    item.color.r * 0.9,
                    item.color.g * 0.9,
                    item.color.b * 0.9,
                    255.0,
                );
                let mut transition = Transition::ease_out(
                    if is_hovered && !pressed { 0.0 } else { 0.5 },
                    TransitionProperties::WIDTH
                        | TransitionProperties::POSITION
                        | TransitionProperties::OVERLAY_COLOR
                        | TransitionProperties::BACKGROUND_COLOR,
                );
                transition.enter = TransitionEnter {
                    initial: Some(slide_and_fade),
                    ..TransitionEnter::default()
                };
                transition.exit = TransitionExit {
                    target: Some(slide_and_fade),
                    ..TransitionExit::default()
                };

                row_node.children.push(
                    Node::new()
                        .id(id)
                        .layout(Layout {
                            sizing: Sizing {
                                width: AxisSize::GROW,
                                height: AxisSize::GROW,
                            },
                            align_x: AlignX::Center,
                            align_y: AlignY::Center,
                            ..Layout::default()
                        })
                        .background(item.color)
                        .overlay(if is_hovered {
                            Color::rgba(140.0, 140.0, 140.0, 80.0)
                        } else {
                            Color::TRANSPARENT
                        })
                        .radius(Radius::all(12.0))
                        .border(Border {
                            color: darker,
                            width: Padding::all(3.0),
                            radius: Radius::all(12.0),
                        })
                        .transition(transition)
                        .child(Node::text(
                            format!("{:02}", item.id),
                            TextStyle {
                                color: if item.id > 29 { WHITE } else { DARK_PURPLE },
                                font_size: 32.0,
                                ..TextStyle::default()
                            },
                        )),
                );
            }
            root.children.push(row_node);
        }
        root
    }
}

fn toolbar(hovered: Option<&str>) -> Node {
    ["shuffle", "blue", "pink", "add"]
        .into_iter()
        .zip(["Randomise", "Blue", "Pink", "Add Box"])
        .fold(
            Node::new()
                .layout(Layout {
                    sizing: Sizing {
                        width: AxisSize::GROW,
                        height: AxisSize::fixed(60.0),
                    },
                    padding: Padding::new(16.0, 0.0, 0.0, 0.0),
                    gap: 16.0,
                    align_y: AlignY::Center,
                    ..Layout::default()
                })
                .background(PURPLE)
                .radius(Radius::all(12.0)),
            |toolbar, (id, label)| {
                toolbar.child(
                    Node::new()
                        .id(id)
                        .layout(Layout {
                            padding: Padding::new(16.0, 16.0, 8.0, 8.0),
                            ..Layout::default()
                        })
                        .background(if hovered == Some(id) {
                            DARK_PURPLE
                        } else {
                            Color::TRANSPARENT
                        })
                        .radius(Radius::all(6.0))
                        .border(Border {
                            color: WHITE,
                            width: Padding::all(2.0),
                            radius: Radius::all(6.0),
                        })
                        .child(Node::text(
                            label,
                            TextStyle {
                                color: WHITE,
                                font_size: 20.0,
                                ..TextStyle::default()
                            },
                        )),
                )
            },
        )
}

fn slide_and_fade(
    mut values: TransitionValues,
    properties: TransitionProperties,
) -> TransitionValues {
    if properties.contains(TransitionProperties::Y) {
        values.bounds.y += 20.0;
    }
    if properties.contains(TransitionProperties::OVERLAY_COLOR) {
        values.overlay = WHITE;
    }
    values
}

fn pink(id: usize) -> Color {
    Color::rgba(
        (255_i32 - id as i32).max(0) as f32,
        (255_i32 - id as i32 * 4).max(0) as f32,
        (255_i32 - id as i32 * 2).max(0) as f32,
        255.0,
    )
}

fn blue(id: usize) -> Color {
    Color::rgba(
        (255_i32 - id as i32 * 4).max(0) as f32,
        (255_i32 - id as i32 * 2).max(0) as f32,
        (255_i32 - id as i32).max(0) as f32,
        255.0,
    )
}

fn window_conf() -> mq::Conf {
    mq::Conf {
        window_title: "Rlay - Transitions".into(),
        window_width: 1024,
        window_height: 768,
        window_resizable: true,
        high_dpi: true,
        sample_count: 4,
        ..mq::Conf::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut engine = Engine::new(macroquad_renderer::measure_text);
    let mut app = App::new();
    let mut previous = rlay::LayoutResult::default();

    loop {
        let (mouse_x, mouse_y) = mq::mouse_position();
        let mouse = Point::new(mouse_x, mouse_y);
        let hovered = Engine::hit_test(&previous, mouse).map(str::to_owned);
        let pressed = mq::is_mouse_button_pressed(mq::MouseButton::Left);

        engine.input_mut().set_mouse_position(mouse);
        let result = engine.layout(
            &app.tree(hovered.as_deref(), pressed),
            Size::new(mq::screen_width(), mq::screen_height()),
            mq::get_frame_time(),
        );
        if pressed && let Some(id) = hovered.as_deref() {
            app.click(id);
        }

        mq::clear_background(mq::BLACK);
        macroquad_renderer::render(&result.commands);
        previous = result;
        mq::next_frame().await;
    }
}
