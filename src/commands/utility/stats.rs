use crate::{Context, Error};
use poise::serenity_prelude as serenity;
use std::time::Instant;

/// Check bot response latency and system status
#[poise::command(prefix_command, track_edits, slash_command, category = "Utility")]
pub async fn stats(
    ctx: Context<'_>,
    #[description = "Show detailed system statistics"] verbose: Option<bool>,
) -> Result<(), Error> {
    let started_at = Instant::now();
    let reply = ctx.say("Calculating statistics...").await?;

    let api_latency = started_at.elapsed().as_millis();
    let gateway_latency = ctx.ping().await.as_millis();

    if verbose.unwrap_or(false) {
        let uptime_secs = ctx.data().start_time.elapsed().as_secs();

        let days = uptime_secs / 86400;
        let hours = (uptime_secs % 86400) / 3600;
        let minutes = (uptime_secs % 3600) / 60;
        let seconds = uptime_secs % 60;

        let uptime_formatted = format!("{days}d {hours}h {minutes}m {seconds}s");
        let active_anims = ctx.data().active_animations.read().await.len();
        let guild_count = ctx.serenity_context().cache.guild_count();

        let embed = serenity::CreateEmbed::new()
            .title("System Status")
            .color(0x2f3136)
            .field("Uptime", uptime_formatted, true)
            .field("Gateway Latency", format!("{gateway_latency} ms"), true)
            .field("API Latency", format!("{api_latency} ms"), true)
            .field("Guilds", guild_count.to_string(), true)
            .field("Active Animations", active_anims.to_string(), true)
            .footer(serenity::CreateEmbedFooter::new("rusty-moss"));

        reply
            .edit(
                ctx,
                poise::CreateReply::default().embed(embed).content(""),
            )
            .await?;
    } else {
        reply
            .edit(
                ctx,
                poise::CreateReply::default().content(format!(
                    "API latency: {api_latency}ms | Gateway latency: {gateway_latency}ms"
                )),
            )
            .await?;
    }

    Ok(())
}
