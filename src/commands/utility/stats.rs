use crate::{Context, Error};
use poise::serenity_prelude as serenity;
use std::time::Instant;

pub fn format_duration(total_secs: u64) -> String {
    let days = total_secs / 86400;
    let hours = (total_secs % 86400) / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    format!("{days}d {hours}h {minutes}m {seconds}s")
}

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
        let uptime_formatted = format_duration(uptime_secs);
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
            .edit(ctx, poise::CreateReply::default().embed(embed).content(""))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "0d 0h 0m 0s");
        assert_eq!(format_duration(65), "0d 0h 1m 5s");
        assert_eq!(format_duration(3665), "0d 1h 1m 5s");
        assert_eq!(format_duration(90065), "1d 1h 1m 5s");
    }
}

