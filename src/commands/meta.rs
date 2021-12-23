use std::collections::HashSet;

use serenity::framework::standard::{macros::command, Args, CommandResult};
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

#[command]
async fn cat(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
	let code = args.single::<u16>()?;
	let status_codes: HashSet<u16> = HashSet::from_iter(vec![100, 101, 102, 200, 201, 202, 203, 204, 206, 207, 
		300, 301, 302, 303, 304, 305, 307, 308, 400, 401, 402, 403, 404, 405, 406, 407, 408, 409, 410, 411, 412, 413, 414, 415,
		416, 417, 418, 420, 421, 422, 423, 424, 425, 426, 429, 431, 444, 450, 451, 497, 498, 499, 500, 501, 502, 503, 504, 506, 
		507, 508, 509, 510, 511, 521, 523, 525, 599]);

	if status_codes.contains(&code) {
		msg.channel_id.say(&ctx.http, format!("https://http.cat/{}.jpg", code)).await?;
	} else {
		msg.channel_id.say(&ctx.http, format!("https://http.cat/404.jpg")).await?;
	}

	Ok(())
}

#[command]
async fn dog(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
	let code = args.single::<u16>()?;
	let status_codes: HashSet<u16> = HashSet::from_iter(vec![100, 200, 201, 202, 203, 204, 206, 207, 208, 226, 
		300, 301, 302, 303, 304, 305, 306, 307, 308, 400, 401, 402, 403, 404, 405, 406, 407, 408, 409, 410, 411, 412, 413, 414, 
		416, 417, 418, 420, 422, 423, 424, 425, 426, 429, 431, 444, 450, 451, 494, 500, 501, 502, 503, 504, 506, 
		507, 508, 509, 510]);

	if status_codes.contains(&code) {
		msg.channel_id.say(&ctx.http, format!("https://httpstatusdogs.com/img/{}.jpg", code)).await?;
	} else {
		msg.channel_id.say(&ctx.http, format!("https://httpstatusdogs.com/img/404.jpg")).await?;
	}

	Ok(())
}