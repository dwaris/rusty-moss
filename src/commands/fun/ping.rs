use crate::{Context, Error};

#[poise::command(prefix_command, slash_command, category = "Fun", user_cooldown = "5")]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.send(|e| e.content(format!("pong"))).await?;

    Ok(())
}

#[poise::command(prefix_command, slash_command, category = "Fun", user_cooldown = "5")]
pub async fn pong(ctx: Context<'_>) -> Result<(), Error> {
    ctx.send(|e| e.content(format!("ping"))).await?;

    Ok(())
}
