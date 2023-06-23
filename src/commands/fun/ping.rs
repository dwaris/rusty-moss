use crate::{Context, Error};

/// Replies with pong
#[poise::command(prefix_command, track_edits, slash_command,  category = "Fun")]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.send(|e| e.content("pong".to_string())).await?;

    Ok(())
}

/// Replies with ping
#[poise::command(prefix_command, track_edits, slash_command, category = "Fun")]
pub async fn pong(ctx: Context<'_>) -> Result<(), Error> {
    ctx.send(|e| e.content("ping".to_string())).await?;
    Ok(())
}
