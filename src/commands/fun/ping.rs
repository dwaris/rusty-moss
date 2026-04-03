use crate::{Context, Error};
use std::time::Instant;

/// Replies with the bot response delay
#[poise::command(prefix_command, track_edits, slash_command,  category = "Fun")]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    let started_at = Instant::now();
    let reply = ctx.say("Pinging...").await?;

    let api_latency_ms = started_at.elapsed().as_millis();
    let gateway_latency = ctx.ping().await.as_millis();

    reply
        .edit(
            ctx,
            poise::CreateReply::default().content(format!(
                "Pong!\nAPI latency: {api_latency_ms}ms\nGateway latency: {gateway_latency}ms"
            )),
        )
        .await?;

    Ok(())
}
