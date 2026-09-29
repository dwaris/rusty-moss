mod shaders;

use crate::{Context, Error};
use poise::serenity_prelude as serenity;
use rand::Rng;
pub use shaders::{
    render_dvd, render_fire, render_matrix, render_rain, render_starfield, wrap_ansi, Particle,
    HEIGHT, WIDTH,
};
use std::time::Duration;
use tokio::time::sleep;

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

        let stars: Vec<Particle> = (0..16)
            .map(|_| Particle {
                x: rng.gen_range(0..WIDTH),
                y: rng.gen_range(0..HEIGHT),
                speed: rng.gen_range(1..=2),
            })
            .collect();

        let rain_drops: Vec<Particle> = (0..16)
            .map(|_| Particle {
                x: rng.gen_range(0..WIDTH),
                y: rng.gen_range(0..HEIGHT),
                speed: rng.gen_range(1..=2),
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
            log::warn!("Failed to edit pixel message: {}", e);
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
