use crate::{Context, Error};
use std::time::Instant;

/// Replies with the bot response delay
#[poise::command(prefix_command, track_edits, slash_command,  category = "Fun")]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    let started_at = Instant::now();
    let reply = ctx.say("Pinging...").await?;

    let response_time = started_at.elapsed().as_millis();
    reply
        .edit(ctx, poise::CreateReply::default().content(format!("Bot response time: {response_time}ms")))
        .await?;

    Ok(())
}
