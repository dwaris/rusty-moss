use crate::{Context, Error};
use poise::serenity_prelude as serenity;
use std::time::Duration;
use tokio::time::sleep;

struct SimpleRng(u64);

impl SimpleRng {
    fn new() -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x853c49e6748fea9b);
        Self(if seed == 0 { 0x853c49e6748fea9b } else { seed })
    }

    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }

    fn range_usize(&mut self, min: usize, max: usize) -> usize {
        if min >= max {
            return min;
        }
        min + (self.next_u64() as usize % (max - min))
    }

    fn range_u8_inclusive(&mut self, min: u8, max: u8) -> u8 {
        if min >= max {
            return min;
        }
        min + (self.next_u64() as u8 % (max - min + 1))
    }

    fn range_f64(&mut self, min: f64, max: f64) -> f64 {
        let frac = (self.next_u64() as f64) / (u64::MAX as f64);
        min + frac * (max - min)
    }
}

const WIDTH: usize = 24;
const HEIGHT: usize = 7;
const STYLE_RESET: &str = "\x1b[0m";
const MATRIX_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789$+-*/=%";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PixelCell {
    ch: char,
    style: u8,
}

impl PixelCell {
    const fn new(ch: char, style: u8) -> Self {
        Self { ch, style }
    }

    const fn blank() -> Self {
        Self { ch: ' ', style: 0 }
    }
}

#[derive(Clone, Copy, Debug)]
struct Particle {
    x: usize,
    y: usize,
    speed: u8,
}

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

fn render_styled_buffer(buffer: &[Vec<PixelCell>], palette: &[&str]) -> String {
    let mut output = String::new();
    let mut current_style: u8 = 0;

    for row in buffer {
        for cell in row {
            if cell.style != current_style {
                let code = palette
                    .get(cell.style as usize)
                    .copied()
                    .unwrap_or(STYLE_RESET);
                output.push_str(code);
                current_style = cell.style;
            }
            output.push(cell.ch);
        }

        if current_style != 0 {
            output.push_str(STYLE_RESET);
            current_style = 0;
        }
        output.push('\n');
    }

    output
}

// --- ANIMATION: MATRIX ---
fn render_matrix(frame: usize, drops: &mut [usize]) -> String {
    let mut buffer = vec![vec![PixelCell::blank(); WIDTH]; HEIGHT];

    for x in 0..WIDTH {
        if x % 2 != 0 {
            continue;
        }

        let speed = (x % 2) + 1;
        let head = (drops[x] + frame * speed) % (HEIGHT + 4);

        for (y, row) in buffer.iter_mut().enumerate() {
            if y == head {
                let char_idx = (frame + x + y) % MATRIX_CHARS.len();
                row[x] = PixelCell::new(MATRIX_CHARS[char_idx] as char, 1);
            } else if head >= y && head - y < 4 {
                let tail_dist = head - y;
                let char_idx = (frame + x + y) % MATRIX_CHARS.len();
                let c = MATRIX_CHARS[char_idx] as char;

                if tail_dist < 2 {
                    row[x] = PixelCell::new(c, 2);
                } else {
                    row[x] = PixelCell::new(c, 3);
                }
            }
        }
    }

    render_styled_buffer(
        &buffer,
        &[STYLE_RESET, "\x1b[1;37m", "\x1b[1;32m", "\x1b[0;32m"],
    )
}

// --- ANIMATION: FIRE ---
fn render_fire(_frame: usize, fire_buffer: &mut [Vec<f64>]) -> String {
    let mut rng = SimpleRng::new();
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

            let cooling = rng.range_f64(0.0, 0.12);
            fire_buffer[y][x] = ((left + right + mid) / 3.0 - cooling).clamp(0.0, 1.0);
        }
    }

    for cell in &mut fire_buffer[HEIGHT - 1] {
        let fuel = rng.range_f64(0.4, 1.0);
        *cell = fuel;
    }

    let mut buffer = vec![vec![PixelCell::blank(); WIDTH]; HEIGHT];

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
            buffer[y][x] = PixelCell::new(c, style);
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

// --- ANIMATION: DVD ---
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

// --- ANIMATION: STARFIELD ---
fn render_starfield(_frame: usize, stars: &mut [Particle]) -> String {
    let mut rng = SimpleRng::new();
    let mut buffer = vec![vec![PixelCell::blank(); WIDTH]; HEIGHT];

    for star in stars.iter_mut() {
        let step = star.speed as usize;

        if star.x < step {
            star.x = WIDTH - 1;
            star.y = rng.range_usize(0, HEIGHT);
        } else {
            star.x -= step;
        }

        let (c, style) = match star.speed {
            2 => ('*', 1),
            _ => ('.', 2),
        };
        buffer[star.y][star.x] = PixelCell::new(c, style);
    }

    render_styled_buffer(&buffer, &[STYLE_RESET, "\x1b[1;37m", "\x1b[0;36m"])
}

// --- ANIMATION: RAIN ---
fn render_rain(_frame: usize, drops: &mut [Particle]) -> String {
    let mut buffer = vec![vec![PixelCell::blank(); WIDTH]; HEIGHT];

    for drop in drops.iter_mut() {
        drop.y = (drop.y + (drop.speed as usize)) % (HEIGHT + 2);

        if drop.y < HEIGHT {
            let (c, style) = match drop.speed {
                2 => ('|', 1),
                _ => ('.', 2),
            };
            buffer[drop.y][drop.x] = PixelCell::new(c, style);
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
        let mut rng = SimpleRng::new();

        let mut drops = vec![0; WIDTH];
        for drop in &mut drops {
            *drop = rng.range_usize(0, HEIGHT);
        }

        let fire_buffer = vec![vec![0.0; WIDTH]; HEIGHT];
        let offset_x = rng.range_usize(0, 15);
        let offset_y = rng.range_usize(0, 5);

        let stars: Vec<Particle> = (0..16)
            .map(|_| Particle {
                x: rng.range_usize(0, WIDTH),
                y: rng.range_usize(0, HEIGHT),
                speed: rng.range_u8_inclusive(1, 2),
            })
            .collect();

        let rain_drops: Vec<Particle> = (0..16)
            .map(|_| Particle {
                x: rng.range_usize(0, WIDTH),
                y: rng.range_usize(0, HEIGHT),
                speed: rng.range_u8_inclusive(1, 2),
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

    let delay = match effect {
        AnimationType::Matrix => Duration::from_millis(600),
        AnimationType::Dvd => Duration::from_millis(600),
        AnimationType::Fire => Duration::from_millis(600),
        AnimationType::Starfield => Duration::from_millis(600),
        AnimationType::Rain => Duration::from_millis(600),
    };

    let mut stopped_manually = false;

    while frame < max_frames {
        if *rx.borrow() {
            stopped_manually = true;
            break;
        }

        let content = match effect {
            AnimationType::Matrix => render_matrix(frame, &mut drops),
            AnimationType::Dvd => render_dvd(frame, offset_x, offset_y),
            AnimationType::Fire => render_fire(frame, &mut fire_buffer),
            AnimationType::Starfield => render_starfield(frame, &mut stars),
            AnimationType::Rain => render_rain(frame, &mut rain_drops),
        };

        let mut edit = serenity::EditMessage::new().content(wrap_ansi(&content));
        if is_endless {
            edit = edit.components(vec![action_row.clone()]);
        }

        if let Err(e) = message.edit(&ctx, edit).await {
            tracing::warn!("Failed to edit pixel message: {}", e);
            break;
        }

        if is_endless {
            let interaction = serenity::ComponentInteractionCollector::new(ctx)
                .author_id(user_id)
                .message_id(message.id)
                .custom_ids(vec![stop_button_id.clone()])
                .timeout(delay)
                .await;

            if let Some(press) = interaction {
                let _ = press
                    .create_response(&ctx, serenity::CreateInteractionResponse::Acknowledge)
                    .await;
                stopped_manually = true;
                break;
            }
        } else {
            sleep(delay).await;
        }

        frame += 1;
    }

    {
        let mut active = ctx.data().active_animations.write().await;
        if let Some(current_tx) = active.get(&user_id)
            && (!*rx.borrow() || current_tx.is_closed())
        {
            active.remove(&user_id);
        }
    }

    let end_content = if stopped_manually {
        "🛑 Animation stopped."
    } else {
        "✨ Animation finished!"
    };

    let _ = message
        .edit(
            &ctx,
            serenity::EditMessage::new()
                .content(end_content)
                .components(vec![]),
        )
        .await;

    Ok(())
}
