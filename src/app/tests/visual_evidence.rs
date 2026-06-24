use super::super::*;
use super::shell_width;
use crate::avatar::{AvatarManager, AvatarState, DEFAULT_CHARACTER};
use crate::chat::Speaker;
use ab_glyph::{Font, FontArc, GlyphId, PxScale, ScaleFont, point};
use eframe::egui::Vec2;
use image::{Rgba, RgbaImage, imageops::FilterType};
use std::path::{Path, PathBuf};

#[test]
#[ignore = "writes Paperclip UX review PNG evidence"]
fn write_visual_evidence_previews() {
    let output_dir = std::env::var_os("DONNA_VISUAL_EVIDENCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/donna-ux-evidence"));
    std::fs::create_dir_all(&output_dir).expect("create visual evidence dir");

    write_visual_preview(
        &output_dir,
        "desktop-default-960x640",
        Vec2::new(960.0, 640.0),
        None,
    );
    write_visual_preview(
        &output_dir,
        "minimum-default-720x480",
        Vec2::new(720.0, 480.0),
        None,
    );
    write_visual_preview(
        &output_dir,
        "after-user-message-960x640",
        Vec2::new(960.0, 640.0),
        Some("Can you help me plan today?"),
    );
    write_visual_preview(
        &output_dir,
        "after-hide-720x480",
        Vec2::new(720.0, 480.0),
        Some("/hide"),
    );
}

fn write_visual_preview(output_dir: &Path, name: &str, size: Vec2, input: Option<&str>) {
    let image = visual_preview(size, input);
    image
        .save(output_dir.join(format!("{name}.png")))
        .expect("save visual evidence");
}

fn visual_preview(size: Vec2, input: Option<&str>) -> RgbaImage {
    let width = size.x.round() as u32;
    let height = size.y.round() as u32;
    let font = FontArc::try_from_slice(epaint_default_fonts::UBUNTU_LIGHT).expect("font");
    let mut image = RgbaImage::from_pixel(width, height, rgb(242, 244, 239));

    draw_text_aligned(
        &mut image,
        &font,
        "Donna",
        Vec2::new(width as f32 / 2.0 - 8.0, 12.0),
        24.0,
        rgb(31, 41, 51),
        TextAlign::Right,
    );
    draw_text(
        &mut image,
        &font,
        "local-first assistant shell",
        width as f32 / 2.0 + 4.0,
        20.0,
        14.0,
        rgb(89, 96, 103),
    );

    let body_top = 68.0;
    let body_available = Vec2::new(size.x, size.y - body_top - 18.0);
    let layout = shell_layout(body_available);
    let messages = preview_messages(input);
    let state = if input == Some("/hide") {
        "Hidden"
    } else {
        "Idle"
    };
    let avatar_state = if input == Some("/hide") {
        AvatarState::Attention
    } else {
        AvatarState::Idle(1)
    };

    if layout.stacked {
        let avatar_outer = layout.avatar_side + AVATAR_FRAME_MARGIN;
        let avatar_x = (size.x - avatar_outer) / 2.0;
        draw_avatar(
            &mut image,
            avatar_x,
            body_top,
            layout.avatar_side,
            avatar_state,
        );

        let chat_outer = layout.chat_width + CHAT_FRAME_MARGIN;
        let chat_x = (size.x - chat_outer) / 2.0;
        draw_chat(
            &mut image,
            &font,
            ChatPreview {
                x: chat_x,
                y: body_top + avatar_outer + layout.gap,
                width: layout.chat_width,
                height: layout.chat_height,
                state,
                messages: &messages,
            },
        );
    } else {
        let x = ((size.x - shell_width(layout)) / 2.0).max(0.0);
        draw_avatar(&mut image, x, body_top, layout.avatar_side, avatar_state);
        draw_chat(
            &mut image,
            &font,
            ChatPreview {
                x: x + layout.avatar_side + AVATAR_FRAME_MARGIN + layout.gap,
                y: body_top,
                width: layout.chat_width,
                height: layout.chat_height,
                state,
                messages: &messages,
            },
        );
    }

    image
}

struct PreviewMessage {
    speaker: Speaker,
    text: &'static str,
}

struct ChatPreview<'a> {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    state: &'static str,
    messages: &'a [PreviewMessage],
}

#[derive(Clone, Copy)]
enum TextAlign {
    Left,
    Right,
    Center,
}

fn preview_messages(input: Option<&str>) -> Vec<PreviewMessage> {
    let mut messages = vec![PreviewMessage {
        speaker: Speaker::Donna,
        text: "Donna is running in local shell mode. Chat stays in memory for this session.",
    }];

    match input {
        Some("/hide") => {
            messages.push(PreviewMessage {
                speaker: Speaker::User,
                text: "/hide",
            });
            messages.push(PreviewMessage {
                speaker: Speaker::Donna,
                text: "Minimizing Donna. If your desktop ignores the request, hide the window from the window manager; Donna keeps running.",
            });
        }
        Some(_) => {
            messages.push(PreviewMessage {
                speaker: Speaker::User,
                text: "Can you help me plan today?",
            });
            messages.push(PreviewMessage {
                speaker: Speaker::Donna,
                text: "Provider integration is not connected yet. I kept this exchange in memory only.",
            });
        }
        None => {}
    }

    messages
}

fn draw_avatar(image: &mut RgbaImage, x: f32, y: f32, side: f32, state: AvatarState) {
    let outer = side + AVATAR_FRAME_MARGIN;
    fill_rect(image, x, y, outer, outer, rgb(232, 236, 230));

    let bytes = AvatarManager::asset_bytes(DEFAULT_CHARACTER, state).expect("embedded avatar");
    let avatar = image::load_from_memory(bytes.as_ref())
        .expect("decode avatar")
        .to_rgba8();
    let image_size = avatar_image_size(
        Vec2::new(avatar.width() as f32, avatar.height() as f32),
        side,
    );
    if image_size.x <= 0.0 || image_size.y <= 0.0 {
        return;
    }

    let image_width = image_size.x.round().max(1.0) as u32;
    let image_height = image_size.y.round().max(1.0) as u32;
    let avatar = image::imageops::resize(&avatar, image_width, image_height, FilterType::Lanczos3);
    overlay(
        image,
        &avatar,
        x + (outer - image_width as f32) / 2.0,
        y + (outer - image_height as f32) / 2.0,
    );
}

fn draw_chat(image: &mut RgbaImage, font: &FontArc, preview: ChatPreview<'_>) {
    let outer_width = preview.width + CHAT_FRAME_MARGIN;
    let outer_height = preview.height + CHAT_FRAME_MARGIN;
    let inner_x = preview.x + 14.0;
    let inner_y = preview.y + 14.0;
    let bar_y = preview.y + outer_height - 86.0;
    fill_rect(
        image,
        preview.x,
        preview.y,
        outer_width,
        outer_height,
        rgb(252, 252, 249),
    );

    let mut message_y = inner_y + 8.0;
    for message in preview.messages {
        let max_width = preview.width * 0.72;
        let lines = wrap_text(font, message.text, max_width, 14.0);
        let bubble_height = 18.0 + lines.len() as f32 * 17.0;
        let bubble_width = bubble_width(font, &lines, 14.0, max_width);
        let bubble_x = match message.speaker {
            Speaker::Donna => inner_x,
            Speaker::User => inner_x + preview.width - bubble_width,
        };
        let fill = match message.speaker {
            Speaker::Donna => rgb(238, 245, 255),
            Speaker::User => rgb(234, 247, 239),
        };

        fill_rect(
            image,
            bubble_x,
            message_y,
            bubble_width,
            bubble_height,
            fill,
        );
        for (index, line) in lines.iter().enumerate() {
            draw_text(
                image,
                font,
                line,
                bubble_x + 12.0,
                message_y + 10.0 + index as f32 * 17.0,
                14.0,
                rgb(31, 41, 51),
            );
        }
        message_y += bubble_height + 8.0;
    }

    fill_rect(
        image,
        inner_x,
        bar_y,
        preview.width,
        1.0,
        rgb(215, 221, 213),
    );
    draw_text(
        image,
        font,
        preview.state,
        inner_x,
        bar_y + 8.0,
        13.0,
        rgb(72, 83, 92),
    );
    draw_text_aligned(
        image,
        font,
        "Ollama local",
        Vec2::new(inner_x + preview.width, bar_y + 8.0),
        13.0,
        rgb(72, 83, 92),
        TextAlign::Right,
    );

    let input_y = bar_y + 34.0;
    let input_width = (preview.width - 74.0).max(96.0);
    fill_rect(
        image,
        inner_x,
        input_y,
        input_width,
        34.0,
        rgb(255, 255, 255),
    );
    stroke_rect(
        image,
        inner_x,
        input_y,
        input_width,
        34.0,
        rgb(184, 193, 180),
    );
    draw_text(
        image,
        font,
        "Message Donna",
        inner_x + 10.0,
        input_y + 9.0,
        14.0,
        rgb(139, 148, 158),
    );

    let send_x = inner_x + preview.width - 64.0;
    fill_rect(image, send_x, input_y, 64.0, 34.0, rgb(238, 241, 236));
    stroke_rect(image, send_x, input_y, 64.0, 34.0, rgb(174, 184, 170));
    draw_text_aligned(
        image,
        font,
        "Send",
        Vec2::new(send_x + 32.0, input_y + 9.0),
        14.0,
        rgb(31, 41, 51),
        TextAlign::Center,
    );
}

fn wrap_text(font: &FontArc, text: &str, max_width: f32, font_size: f32) -> Vec<String> {
    let available = (max_width - 24.0).max(40.0);
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_owned()
        } else {
            format!("{current} {word}")
        };

        if text_width(font, &candidate, font_size) > available && !current.is_empty() {
            lines.push(current);
            current = word.to_owned();
        } else {
            current = candidate;
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}

fn bubble_width(font: &FontArc, lines: &[String], font_size: f32, max_width: f32) -> f32 {
    let text_width = lines
        .iter()
        .map(|line| text_width(font, line, font_size))
        .fold(0.0, f32::max);
    (text_width + 24.0).min(max_width)
}

fn text_width(font: &FontArc, text: &str, font_size: f32) -> f32 {
    let scaled = font.as_scaled(PxScale::from(font_size));
    let mut width = 0.0;
    let mut previous = None;

    for ch in text.chars() {
        let glyph = scaled.glyph_id(ch);
        if let Some(previous) = previous {
            width += scaled.kern(previous, glyph);
        }
        width += scaled.h_advance(glyph);
        previous = Some(glyph);
    }

    width
}

fn draw_text(
    image: &mut RgbaImage,
    font: &FontArc,
    text: &str,
    x: f32,
    y: f32,
    size: f32,
    color: Rgba<u8>,
) {
    draw_text_aligned(
        image,
        font,
        text,
        Vec2::new(x, y),
        size,
        color,
        TextAlign::Left,
    );
}

fn draw_text_aligned(
    image: &mut RgbaImage,
    font: &FontArc,
    text: &str,
    position: Vec2,
    size: f32,
    color: Rgba<u8>,
    align: TextAlign,
) {
    let scaled = font.as_scaled(PxScale::from(size));
    let mut caret = match align {
        TextAlign::Left => position.x,
        TextAlign::Right => position.x - text_width(font, text, size),
        TextAlign::Center => position.x - text_width(font, text, size) / 2.0,
    };
    let baseline = position.y + scaled.ascent();
    let mut previous: Option<GlyphId> = None;

    for ch in text.chars() {
        let glyph_id = scaled.glyph_id(ch);
        if let Some(previous) = previous {
            caret += scaled.kern(previous, glyph_id);
        }

        let glyph = glyph_id.with_scale_and_position(PxScale::from(size), point(caret, baseline));
        if let Some(outlined) = font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            outlined.draw(|gx, gy, coverage| {
                blend_pixel(
                    image,
                    bounds.min.x as i32 + gx as i32,
                    bounds.min.y as i32 + gy as i32,
                    Rgba([color[0], color[1], color[2], (coverage * 255.0) as u8]),
                );
            });
        }

        caret += scaled.h_advance(glyph_id);
        previous = Some(glyph_id);
    }
}

fn fill_rect(image: &mut RgbaImage, x: f32, y: f32, width: f32, height: f32, color: Rgba<u8>) {
    let x0 = x.round().max(0.0) as u32;
    let y0 = y.round().max(0.0) as u32;
    let x1 = (x + width).round().min(image.width() as f32).max(0.0) as u32;
    let y1 = (y + height).round().min(image.height() as f32).max(0.0) as u32;

    for py in y0..y1 {
        for px in x0..x1 {
            image.put_pixel(px, py, color);
        }
    }
}

fn stroke_rect(image: &mut RgbaImage, x: f32, y: f32, width: f32, height: f32, color: Rgba<u8>) {
    fill_rect(image, x, y, width, 1.0, color);
    fill_rect(image, x, y + height - 1.0, width, 1.0, color);
    fill_rect(image, x, y, 1.0, height, color);
    fill_rect(image, x + width - 1.0, y, 1.0, height, color);
}

fn overlay(base: &mut RgbaImage, image: &RgbaImage, x: f32, y: f32) {
    let x = x.round() as i32;
    let y = y.round() as i32;

    for py in 0..image.height() {
        for px in 0..image.width() {
            blend_pixel(base, x + px as i32, y + py as i32, *image.get_pixel(px, py));
        }
    }
}

fn blend_pixel(image: &mut RgbaImage, x: i32, y: i32, source: Rgba<u8>) {
    if x < 0 || y < 0 || x >= image.width() as i32 || y >= image.height() as i32 {
        return;
    }

    let alpha = source[3] as f32 / 255.0;
    let target = image.get_pixel_mut(x as u32, y as u32);
    for channel in 0..3 {
        target[channel] =
            (source[channel] as f32 * alpha + target[channel] as f32 * (1.0 - alpha)).round() as u8;
    }
    target[3] = 255;
}

fn rgb(red: u8, green: u8, blue: u8) -> Rgba<u8> {
    Rgba([red, green, blue, 255])
}
