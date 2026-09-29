use crate::{Context, Error};
use poise::serenity_prelude as serenity;
use rand::Rng;
use std::time::Duration;
use tokio::time::sleep;

const WIDTH: usize = 24;
const HEIGHT: usize = 7;
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
    #[name = "starfield"]
    Starfield,
    #[name = "rain"]
    Rain,
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

// --- SHADER: MATRIX ---
fn render_matrix(frame: usize, drops: &mut [usize]) -> String {
    let mut buffer = vec![vec![(' ', 0u8); WIDTH]; HEIGHT];

    for x in 0..WIDTH {
        if x % 2 != 0 {
            continue;
        }

        let speed = (x % 2) + 1;
        let head = (drops[x] + frame * speed) % (HEIGHT + 4);

        for (y, row) in buffer.iter_mut().enumerate() {
            if y == head {
                let char_idx = (frame + x + y) % MATRIX_CHARS.len();
                row[x] = (MATRIX_CHARS[char_idx] as char, 1);
            } else if head >= y && head - y < 4 {
                let tail_dist = head - y;
                let char_idx = (frame + x + y) % MATRIX_CHARS.len();
                let c = MATRIX_CHARS[char_idx] as char;

                if tail_dist < 2 {
                    row[x] = (c, 2);
                } else {
                    row[x] = (c, 3);
                }
            }
        }
    }

    render_styled_buffer(
        &buffer,
        &[STYLE_RESET, "\x1b[1;37m", "\x1b[1;32m", "\x1b[0;32m"],
    )
}

// --- SHADER: FIRE ---
fn render_fire(_frame: usize, fire_buffer: &mut [Vec<f64>]) -> String {
    let mut rng = rand::thread_rng();
    for y in 0..HEIGHT - 1 {
        for x in 0..WIDTH {
            let left = if x > 0 {
                fire_buffer[y + 1][x - 1]
            } else {
                0.0
            };
            let right = if x < WIDTH - 1 {
                fire_buffer[y + 1][x + 1]
            } else {
                0.0
            };
            let mid = fire_buffer[y + 1][x];

            let cooling = rng.gen_range(0.0..0.12);
            fire_buffer[y][x] = ((left + right + mid) / 3.0 - cooling).clamp(0.0, 1.0);
        }
    }

    for cell in &mut fire_buffer[HEIGHT - 1] {
        let fuel = rng.gen_range(0.4..1.0);
        *cell = fuel;
    }

    let mut buffer = vec![vec![(' ', 0u8); WIDTH]; HEIGHT];

    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let heat = fire_buffer[y][x];
            let (c, style) = if heat > 0.75 {
                ('#', 1)
            } else if heat > 0.45 {
                ('*', 2)
            } else if heat > 0.25 {
                (':', 3)
            } else if heat > 0.10 {
                ('.', 4)
            } else {
                (' ', 0)
            };
            buffer[y][x] = (c, style);
        }
    }

    render_styled_buffer(
        &buffer,
        &[
            STYLE_RESET,
            "\x1b[1;37m",
            "\x1b[1;33m",
            "\x1b[1;31m",
            "\x1b[0;31m",
        ],
    )
}

// --- SHADER: DVD ---
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
    if x >= bounce_limit_x {
        x = bounce_limit_x * 2 - x;
    }

    let mut y = total_dist_y % (bounce_limit_y * 2);
    if y >= bounce_limit_y {
        y = bounce_limit_y * 2 - y;
    }

    let bounces = (total_dist_x / bounce_limit_x) + (total_dist_y / bounce_limit_y);

    let colors = [
        "\x1b[1;31m",
        "\x1b[1;32m",
        "\x1b[1;33m",
        "\x1b[1;34m",
        "\x1b[1;35m",
        "\x1b[1;36m",
    ];
    let color = colors[bounces % colors.len()];

    let mut output = String::new();
    output.push('+');
    for _ in 0..inner_w {
        output.push('-');
    }
    output.push_str("+\n");

    for iy in 0..inner_h {
        output.push('|');
        if iy == y {
            for _ in 0..x {
                output.push(' ');
            }
            output.push_str(color);
            output.push_str("DVD");
            output.push_str("\x1b[0m");
            for _ in (x + 3)..inner_w {
                output.push(' ');
            }
        } else {
            for _ in 0..inner_w {
                output.push(' ');
            }
        }
        output.push_str("|\n");
    }

    output.push('+');
    for _ in 0..inner_w {
        output.push('-');
    }
    output.push_str("+\n");
    output
}

// --- SHADER: STARFIELD (SIMPLE) ---
fn render_starfield(_frame: usize, stars: &mut [(usize, usize, u8)]) -> String {
    let mut buffer = vec![vec![(' ', 0u8); WIDTH]; HEIGHT];

    for star in stars.iter_mut() {
        let (x, y, speed) = star;
        let step = *speed as usize;

        if *x < step {
            *x = WIDTH - 1;
            *y = rand::thread_rng().gen_range(0..HEIGHT);
        } else {
            *x -= step;
        }

        let (c, style) = match *speed {
            2 => ('*', 1),
            _ => ('.', 2),
        };
        buffer[*y][*x] = (c, style);
    }

    render_styled_buffer(&buffer, &[STYLE_RESET, "\x1b[1;37m", "\x1b[0;36m"])
}

// --- SHADER: RAIN (SIMPLE) ---
fn render_rain(_frame: usize, drops: &mut [(usize, usize, u8)]) -> String {
    let mut buffer = vec![vec![(' ', 0u8); WIDTH]; HEIGHT];

    for drop in drops.iter_mut() {
        let (x, y, speed) = drop;
        *y = (*y + (*speed as usize)) % (HEIGHT + 2);

        if *y < HEIGHT {
            let (c, style) = match *speed {
                2 => ('|', 1),
                _ => ('.', 2),
            };
            buffer[*y][*x] = (c, style);
        }
    }

    render_styled_buffer(&buffer, &[STYLE_RESET, "\x1b[1;36m", "\x1b[0;36m"])
}

/// Stop any ongoing animation you have running
#[poise::command(slash_command, prefix_command, category = "Fun")]
pub async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    let user_id = ctx.author().id;
    let mut active = ctx.data().active_animations.write().await;

    if let Some(tx) = active.remove(&user_id) {
        let _ = tx.send(true);
        ctx.say("Animation stopped.").await?;
    } else {
        ctx.say("You don't have any animations running.").await?;
    }

    Ok(())
}

/// Run a cool ASCII pixel animation
#[poise::command(slash_command, prefix_command, user_cooldown = 5, category = "Fun")]
pub async fn pixel(
    ctx: Context<'_>,
    #[description = "The animation effect to play"] effect: AnimationType,
    #[description = "Run for a long time (max 2 mins)"] endless: Option<bool>,
) -> Result<(), Error> {
    let user_id = ctx.author().id;
    let (tx, rx) = tokio::sync::watch::channel(false);

    {
        let mut active = ctx.data().active_animations.write().await;
        if let Some(old_tx) = active.insert(user_id, tx) {
            let _ = old_tx.send(true);
        }
    }

    let (mut drops, mut fire_buffer, offset_x, offset_y, mut stars, mut rain_drops) = {
        let mut rng = rand::thread_rng();

        let mut drops = vec![0; WIDTH];
        for drop in &mut drops {
            *drop = rng.gen_range(0..HEIGHT);
        }

        let fire_buffer = vec![vec![0.0; WIDTH]; HEIGHT];
        let offset_x = rng.gen_range(0..15);
        let offset_y = rng.gen_range(0..5);

        let stars: Vec<(usize, usize, u8)> = (0..16)
            .map(|_| {
                (
                    rng.gen_range(0..WIDTH),
                    rng.gen_range(0..HEIGHT),
                    rng.gen_range(1..=2),
                )
            })
            .collect();

        let rain_drops: Vec<(usize, usize, u8)> = (0..16)
            .map(|_| {
                (
                    rng.gen_range(0..WIDTH),
                    rng.gen_range(0..HEIGHT),
                    rng.gen_range(1..=2),
                )
            })
            .collect();

        (drops, fire_buffer, offset_x, offset_y, stars, rain_drops)
    };

    let mut frame = 0;
    let is_endless = endless.unwrap_or(false);
    let max_frames = if is_endless { 100 } else { 10 };

    let stop_button_id = format!("stop_pixel_{}", user_id);
    let stop_button = serenity::CreateButton::new(&stop_button_id)
        .label("Stop Animation")
        .style(serenity::ButtonStyle::Secondary);
    let action_row = serenity::CreateActionRow::Buttons(vec![stop_button]);

    let mut message = ctx
        .say(wrap_ansi("Initializing..."))
        .await?
        .into_message()
        .await?;

    while frame < max_frames {
        if *rx.borrow() {
            break;
        }

        let content = match effect {
            AnimationType::Matrix => render_matrix(frame, &mut drops),
            AnimationType::Dvd => render_dvd(frame, offset_x, offset_y),
            AnimationType::Fire => render_fire(frame, &mut fire_buffer),
            AnimationType::Starfield => render_starfield(frame, &mut stars),
            AnimationType::Rain => render_rain(frame, &mut rain_drops),
        };

        if message
            .edit(
                ctx.serenity_context(),
                serenity::EditMessage::new()
                    .content(wrap_ansi(&content))
                    .components(vec![action_row.clone()]),
            )
            .await
            .is_err()
        {
            break;
        }

        frame += 1;

        let collector = message.await_component_interaction(ctx.serenity_context());
        tokio::select! {
            _ = sleep(Duration::from_millis(1250)) => {},
            interaction = collector => {
                if let Some(press) = interaction
                    && press.data.custom_id == stop_button_id
                {
                    let _ = press.create_response(
                        ctx.serenity_context(),
                        serenity::CreateInteractionResponse::Acknowledge,
                    ).await;
                    break;
                }
            }
        }
    }

    {
        let mut active = ctx.data().active_animations.write().await;
        if let Some(current_tx) = active.get(&user_id)
            && (!*rx.borrow() || current_tx.is_closed() || frame >= max_frames)
        {
            active.remove(&user_id);
        }
    }

    let _ = message
        .edit(
            ctx.serenity_context(),
            serenity::EditMessage::new()
                .content("Animation finished.")
                .components(vec![]),
        )
        .await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tokio::sync::RwLock;

    #[tokio::test]
    async fn test_active_animation_registration_and_cleanup() {
        let active_animations = RwLock::new(HashMap::new());
        let user_id = serenity::UserId::new(123456789);

        let (tx, rx) = tokio::sync::watch::channel(false);

        {
            let mut active = active_animations.write().await;
            if let Some(old_tx) = active.insert(user_id, tx) {
                let _ = old_tx.send(true);
            }
        }

        assert_eq!(active_animations.read().await.len(), 1);
        assert!(!*rx.borrow());

        {
            let mut active = active_animations.write().await;
            if let Some(current_tx) = active.get(&user_id)
                && (!*rx.borrow() || current_tx.is_closed())
            {
                active.remove(&user_id);
            }
        }

        assert_eq!(active_animations.read().await.len(), 0);
        assert!(!active_animations.read().await.contains_key(&user_id));
    }
}
