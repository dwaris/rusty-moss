use crate::{BotData, Context, Error};
use std::collections::BTreeMap;
use std::fmt::Write as _;

const EXTRA_HELP_TEXT: &str =
    "This is a bot made by the ITClowd, for the ITClowd.\nWritten in Rust with the help of poise.";

/// Show this help menu
#[poise::command(prefix_command, track_edits, category = "Utility", slash_command)]
pub async fn help(
    ctx: Context<'_>,
    #[description = "Specific command to show help about"]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    command: Option<String>,
) -> Result<(), Error> {
    if let Some(cmd_name) = command {
        let commands = &ctx.framework().options().commands;
        let found = commands
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(&cmd_name));

        if let Some(cmd) = found {
            let mut text = String::new();
            let prefix = ctx.prefix();
            let _ = writeln!(text, "**`{}{}`**", prefix, cmd.name);

            if let Some(desc) = &cmd.description {
                let _ = writeln!(text, "\n{}", desc);
            }
            if let Some(help_text) = &cmd.help_text {
                let _ = writeln!(text, "\n{}", help_text);
            }

            if !cmd.parameters.is_empty() {
                text.push_str("\n**Parameters:**\n```\n");
                for param in cmd.parameters.iter() {
                    let req = if param.required {
                        "required"
                    } else {
                        "optional"
                    };
                    let desc = param.description.as_deref().unwrap_or("");
                    let _ = writeln!(text, "{:15} ({}) {}", param.name, req, desc);
                }
                text.push_str("```\n");
            }

            ctx.say(text).await?;
        } else {
            ctx.say(format!("No such command `{}`", cmd_name)).await?;
        }
    } else {
        let mut categories: BTreeMap<&str, Vec<&poise::Command<BotData, Error>>> = BTreeMap::new();
        for cmd in &ctx.framework().options().commands {
            if !cmd.hide_in_help {
                let cat = cmd.category.as_deref().unwrap_or("Commands");
                categories.entry(cat).or_default().push(cmd);
            }
        }

        let prefix = ctx.prefix();
        let mut menu = String::from("```\n");

        for (category, commands) in categories {
            let _ = writeln!(menu, "{}:", category);
            for cmd in commands {
                let desc = cmd.description.as_deref().unwrap_or("");
                let cmd_display = format!("  {}{}", prefix, cmd.name);
                let _ = writeln!(menu, "{:18} {}", cmd_display, desc);
            }
            menu.push('\n');
        }

        let _ = writeln!(menu, "{}", EXTRA_HELP_TEXT);
        menu.push_str("```");

        ctx.say(menu).await?;
    }

    Ok(())
}
