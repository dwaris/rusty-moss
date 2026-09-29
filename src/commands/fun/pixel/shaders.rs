use rand::Rng;

pub const WIDTH: usize = 24;
pub const HEIGHT: usize = 7;
pub const STYLE_RESET: &str = "\x1b[0m";
pub const MATRIX_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789$+-*/=%";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelCell {
    pub ch: char,
    pub style: u8,
}

impl PixelCell {
    pub const fn new(ch: char, style: u8) -> Self {
        Self { ch, style }
    }

    pub const fn blank() -> Self {
        Self { ch: ' ', style: 0 }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub x: usize,
    pub y: usize,
    pub speed: u8,
}

pub fn wrap_ansi(content: &str) -> String {
    format!("```ansi\n{}\n```", content)
}

pub fn render_styled_buffer(buffer: &[Vec<PixelCell>], palette: &[&str]) -> String {
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

// --- SHADER: MATRIX ---
pub fn render_matrix(frame: usize, drops: &mut [usize]) -> String {
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

// --- SHADER: FIRE ---
pub fn render_fire(_frame: usize, fire_buffer: &mut [Vec<f64>]) -> String {
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

// --- SHADER: DVD ---
pub fn render_dvd(frame: usize, offset_x: usize, offset_y: usize) -> String {
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
pub fn render_starfield(_frame: usize, stars: &mut [Particle]) -> String {
    let mut buffer = vec![vec![PixelCell::blank(); WIDTH]; HEIGHT];

    for star in stars.iter_mut() {
        let step = star.speed as usize;

        if star.x < step {
            star.x = WIDTH - 1;
            star.y = rand::thread_rng().gen_range(0..HEIGHT);
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

// --- SHADER: RAIN (SIMPLE) ---
pub fn render_rain(_frame: usize, drops: &mut [Particle]) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_styled_buffer_handles_out_of_bounds_style() {
        let buffer = vec![vec![
            PixelCell::new('A', 1),
            PixelCell::new('B', 99), // out of bounds palette index
        ]];
        let palette = &[STYLE_RESET, "\x1b[1;31m"];
        let rendered = render_styled_buffer(&buffer, palette);
        assert!(rendered.contains('A'));
        assert!(rendered.contains('B'));
    }
}
