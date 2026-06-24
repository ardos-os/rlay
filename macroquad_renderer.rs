use macroquad::prelude as mq;
use rlay::{Color, CommandKind, RenderCommand, Size, TextStyle};

pub fn measure_text(text: &str, style: &TextStyle) -> Size {
    let size = mq::measure_text(text, None, style.font_size as u16, 1.0);
    Size::new(size.width, size.height)
}

pub fn render(commands: &[RenderCommand]) {
    let mut overlays = Vec::new();
    for command in commands {
        let bounds = command.bounds;
        match &command.kind {
            CommandKind::Rectangle { color, radius } => {
                rounded_rectangle(bounds, radius.top_left, mq_color(*color));
            }
            CommandKind::Border(border) => {
                rounded_border(
                    bounds,
                    border.radius.top_left,
                    border.width.left,
                    mq_color(border.color),
                );
            }
            CommandKind::Text { text, style } => {
                let metrics = mq::measure_text(text, None, style.font_size as u16, 1.0);
                mq::draw_text_ex(
                    text,
                    bounds.x,
                    bounds.y + (bounds.height - metrics.height) / 2.0 + metrics.offset_y,
                    mq::TextParams {
                        font_size: style.font_size as u16,
                        color: mq_color(style.color),
                        ..mq::TextParams::default()
                    },
                );
            }
            CommandKind::OverlayStart(color) => overlays.push((bounds, *color)),
            CommandKind::OverlayEnd => {
                if let Some((bounds, color)) = overlays.pop() {
                    rounded_rectangle(bounds, 12.0, mq_color(color));
                }
            }
            _ => {}
        }
    }
}

fn rounded_border(bounds: rlay::Rect, radius: f32, width: f32, color: mq::Color) {
    let radius = radius.min(bounds.width / 2.0).min(bounds.height / 2.0);
    let width = width
        .min(bounds.width / 2.0)
        .min(bounds.height / 2.0)
        .max(0.0);
    if width == 0.0 {
        return;
    }
    if radius == 0.0 {
        mq::draw_rectangle(bounds.x, bounds.y, bounds.width, width, color);
        mq::draw_rectangle(
            bounds.x,
            bounds.y + bounds.height - width,
            bounds.width,
            width,
            color,
        );
        mq::draw_rectangle(
            bounds.x,
            bounds.y + width,
            width,
            bounds.height - width * 2.0,
            color,
        );
        mq::draw_rectangle(
            bounds.x + bounds.width - width,
            bounds.y + width,
            width,
            bounds.height - width * 2.0,
            color,
        );
        return;
    }
    let width = width.min(radius);

    mq::draw_rectangle(
        bounds.x + radius,
        bounds.y,
        bounds.width - radius * 2.0,
        width,
        color,
    );
    mq::draw_rectangle(
        bounds.x + radius,
        bounds.y + bounds.height - width,
        bounds.width - radius * 2.0,
        width,
        color,
    );
    mq::draw_rectangle(
        bounds.x,
        bounds.y + radius,
        width,
        bounds.height - radius * 2.0,
        color,
    );
    mq::draw_rectangle(
        bounds.x + bounds.width - width,
        bounds.y + radius,
        width,
        bounds.height - radius * 2.0,
        color,
    );

    for (x, y, start_angle) in [
        (bounds.x + radius, bounds.y + radius, 180.0),
        (bounds.x + bounds.width - radius, bounds.y + radius, 270.0),
        (
            bounds.x + bounds.width - radius,
            bounds.y + bounds.height - radius,
            0.0,
        ),
        (bounds.x + radius, bounds.y + bounds.height - radius, 90.0),
    ] {
        quarter_ring(x, y, radius, width, start_angle, color);
    }
}

fn quarter_ring(x: f32, y: f32, radius: f32, width: f32, start_angle: f32, color: mq::Color) {
    const SEGMENTS: usize = 8;
    let center = mq::vec2(x, y);
    let inner_radius = radius - width;
    for segment in 0..SEGMENTS {
        let angle = |offset: usize| {
            (start_angle + 90.0 * (segment + offset) as f32 / SEGMENTS as f32).to_radians()
        };
        let point = |angle: f32, radius: f32| center + mq::vec2(angle.cos(), angle.sin()) * radius;
        let outer_a = point(angle(0), radius);
        let outer_b = point(angle(1), radius);
        let inner_a = point(angle(0), inner_radius);
        let inner_b = point(angle(1), inner_radius);
        mq::draw_triangle(outer_a, outer_b, inner_b, color);
        mq::draw_triangle(outer_a, inner_b, inner_a, color);
    }
}

fn rounded_rectangle(bounds: rlay::Rect, radius: f32, color: mq::Color) {
    let radius = radius.min(bounds.width / 2.0).min(bounds.height / 2.0);
    if radius <= 0.0 {
        mq::draw_rectangle(bounds.x, bounds.y, bounds.width, bounds.height, color);
        return;
    }

    mq::draw_rectangle(
        bounds.x + radius,
        bounds.y,
        bounds.width - radius * 2.0,
        bounds.height,
        color,
    );
    mq::draw_rectangle(
        bounds.x,
        bounds.y + radius,
        radius,
        bounds.height - radius * 2.0,
        color,
    );
    mq::draw_rectangle(
        bounds.x + bounds.width - radius,
        bounds.y + radius,
        radius,
        bounds.height - radius * 2.0,
        color,
    );

    for (x, y, start_angle) in [
        (bounds.x + radius, bounds.y + radius, 180.0),
        (bounds.x + bounds.width - radius, bounds.y + radius, 270.0),
        (
            bounds.x + bounds.width - radius,
            bounds.y + bounds.height - radius,
            0.0,
        ),
        (bounds.x + radius, bounds.y + bounds.height - radius, 90.0),
    ] {
        quarter_circle(x, y, radius, start_angle, color);
    }
}

fn quarter_circle(x: f32, y: f32, radius: f32, start_angle: f32, color: mq::Color) {
    const SEGMENTS: usize = 8;
    let center = mq::vec2(x, y);
    for segment in 0..SEGMENTS {
        let angle = |offset: usize| {
            (start_angle + 90.0 * (segment + offset) as f32 / SEGMENTS as f32).to_radians()
        };
        let point = |angle: f32| center + mq::vec2(angle.cos(), angle.sin()) * radius;
        mq::draw_triangle(center, point(angle(0)), point(angle(1)), color);
    }
}

fn mq_color(color: Color) -> mq::Color {
    mq::Color::from_rgba(
        color.r.clamp(0.0, 255.0) as u8,
        color.g.clamp(0.0, 255.0) as u8,
        color.b.clamp(0.0, 255.0) as u8,
        color.a.clamp(0.0, 255.0) as u8,
    )
}
