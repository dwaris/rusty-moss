use crate::{Context, Error};

/// Show this help menu
#[poise::command(prefix_command, track_edits, category = "Utility", slash_command)]
pub async fn help(
    ctx: Context<'_>,
    #[description = "Specific command to show help about"]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    command: Option<String>,
) -> Result<(), Error> {
    poise::builtins::help(
        ctx,
        command.as_deref(),
        poise::builtins::HelpConfiguration {
            extra_text_at_bottom: "This is an bot made by the ITClowd, for the ITClowd.\nWritten in Rust with the help of poise.",
            ..Default::default()
        },
    )
    .await?;
    Ok(())
}
