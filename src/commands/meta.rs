use serenity::framework::standard::{macros::command, CommandResult};
use serenity::model::prelude::*;
use serenity::prelude::*;

#[command]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult {
	msg.channel_id.say(&ctx.http, "pong").await?;

	Ok(())
}

#[command]
async fn pong(ctx: &Context, msg: &Message) -> CommandResult {
	msg.channel_id.say(&ctx.http, "ping").await?;

	Ok(())
}