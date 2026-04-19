use crate::{Context, Error};
use poise::serenity_prelude as serenity;
use std::time::Duration;
use tokio::time::sleep;

const WIDTH: usize = 30;
const HEIGHT: usize = 12;
const STYLE_RESET: &str = "\x1b[0m";
const MATRIX_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789$+-*/=%";

#[derive(poise::ChoiceParameter)]
pub enum AnimationType {
    #[name = "matrix"]
    Matrix,
    #[name = "dvd"]
    Dvd,
    #[name = "fire"]
    Fire,
}

fn wrap_ansi(content: &str) -> String {
    format!("```ansi\n{}\n```", content)
}

fn render_styled_buffer(buffer: &[Vec<(char, u8)>], palette: &[&str]) -> String {
    let mut output = String::new();
    let mut current_style: u8 = 0;

    for row in buffer {
        for (ch, style) in row {
            if *style != current_style {
                output.push_str(palette[*style as usize]);
                current_style = *style;
            }
            output.push(*ch);
        }

        if current_style != 0 {
            output.push_str(STYLE_RESET);
            current_style = 0;
        }
        output.push('\n');
    }

    output
}

// --- MATRIX ---
fn render_matrix(frame: usize, drops: &mut [usize]) -> String {
    let mut buffer = vec![vec![(' ', 0u8); WIDTH]; HEIGHT];
    
    for x in 0..WIDTH {
        if x % 2 != 0 {
            continue;
        }

        let speed = (x % 3) + 1;
        let head = (drops[x] + frame * speed / 2) % (HEIGHT + 10);

        for y in 0..HEIGHT {
            if y == head {
                let char_idx = (frame + x + y) % MATRIX_CHARS.len();
                buffer[y][x] = (MATRIX_CHARS[char_idx] as char, 1);
            } else if head >= y && head - y < 8 {
                let tail_dist = head - y;
                let char_idx = (frame + x + y) % MATRIX_CHARS.len();
                let c = MATRIX_CHARS[char_idx] as char;

                if tail_dist < 3 {
                    buffer[y][x] = (c, 2);
                } else {
                    buffer[y][x] = (c, 3);
                }
            }
        }
    }

    render_styled_buffer(&buffer, &[STYLE_RESET, "\x1b[1;37m", "\x1b[1;32m", "\x1b[0;32m"])
}

// --- FIRE ---
fn render_fire(_frame: usize, fire_buffer: &mut [Vec<f64>]) -> String {
    for y in 0..HEIGHT - 1 {
        for x in 0..WIDTH {
            let left = if x > 0 { fire_buffer[y + 1][x - 1] } else { 0.0 };
            let right = if x < WIDTH - 1 { fire_buffer[y + 1][x + 1] } else { 0.0 };
            let mid = fire_buffer[y + 1][x];
            
            let cooling = rand::random::<f64>() * 0.1;
            fire_buffer[y][x] = ((left + right + mid) / 3.0 - cooling).clamp(0.0, 1.0);
        }
    }

    for x in 0..WIDTH {
        let fuel = (rand::random::<f64>() * 0.5) + 0.5;
        fire_buffer[HEIGHT - 1][x] = fuel;
    }

    let mut buffer = vec![vec![(' ', 0u8); WIDTH]; HEIGHT];

    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let heat = fire_buffer[y][x];
            let (c, style) = if heat > 0.8 {
                ('W', 1)
            } else if heat > 0.5 {
                ('M', 2)
            } else if heat > 0.3 {
                ('w', 3)
            } else if heat > 0.1 {
                ('.', 4)
            } else {
                (' ', 0)
            };
            buffer[y][x] = (c, style);
        }
    }

    render_styled_buffer(&buffer, &[STYLE_RESET, "\x1b[1;37m", "\x1b[1;33m", "\x1b[1;31m", "\x1b[0;31m"])
}

// --- DVD ---
fn render_dvd(frame: usize, offset_x: usize, offset_y: usize) -> String {
    let speed_x = 2;
    let speed_y = 1;

    let inner_w = WIDTH - 2;
    let inner_h = HEIGHT - 2;

    let bounce_limit_x = inner_w - 3;
    let bounce_limit_y = inner_h - 1;

    let total_dist_x = frame * speed_x + offset_x;
    let total_dist_y = frame * speed_y + offset_y;

    let mut x = total_dist_x % (bounce_limit_x * 2);
    if x >= bounce_limit_x { x = bounce_limit_x * 2 - x; }
    
    let mut y = total_dist_y % (bounce_limit_y * 2);
    if y >= bounce_limit_y { y = bounce_limit_y * 2 - y; }

    let bounces = (total_dist_x / bounce_limit_x) + (total_dist_y / bounce_limit_y);

    let colors = ["\x1b[1;31m", "\x1b[1;32m", "\x1b[1;33m", "\x1b[1;34m", "\x1b[1;35m", "\x1b[1;36m"];
    let color = colors[bounces % colors.len()];
    
    let mut output = String::new();
    output.push('+');
    for _ in 0..inner_w { output.push('-'); }
    output.push_str("+\n");

    for iy in 0..inner_h {
        output.push('|');
        if iy == y {
            for _ in 0..x { output.push(' '); }
            output.push_str(color);
            output.push_str("DVD");
            output.push_str("\x1b[0m");
            for _ in (x+3)..inner_w { output.push(' '); }
        } else {
            for _ in 0..inner_w { output.push(' '); }
        }
        output.push_str("|\n");
    }

    output.push('+');
    for _ in 0..inner_w { output.push('-'); }
    output.push_str("+\n");
    output
}

#[poise::command(slash_command, prefix_command, category = "Fun")]
pub async fn pixel(
    ctx: Context<'_>,
    #[description = "The animation effect to play"] effect: AnimationType,
    #[description = "Run for a long time (max 5.5 mins)"] endless: Option<bool>,
) -> Result<(), Error> {
    let mut drops = vec![0; WIDTH];
    for drop in &mut drops {
        *drop = rand::random::<usize>() % HEIGHT;
    }

    let mut fire_buffer = vec![vec![0.0; WIDTH]; HEIGHT];

    let offset_x = rand::random::<usize>() % 20;
    let offset_y = rand::random::<usize>() % 10;

    let mut frame = 0;
    let max_frames = if endless.unwrap_or(false) { 300 } else { 15 };

    let mut message = ctx.say(wrap_ansi("Initializing shader...")).await?.into_message().await?;

    while frame < max_frames {
        let content = match effect {
            AnimationType::Matrix => render_matrix(frame, &mut drops),
            AnimationType::Dvd => render_dvd(frame, offset_x, offset_y),
            AnimationType::Fire => render_fire(frame, &mut fire_buffer),
        };

        message.edit(ctx.serenity_context(), serenity::EditMessage::new()
            .content(wrap_ansi(&content))
        ).await?;

        frame += 1;
        sleep(Duration::from_millis(1100)).await;
    }

    message.edit(ctx.serenity_context(), serenity::EditMessage::new()
        .content("Animation finished.")
    ).await?;

    Ok(())
}
