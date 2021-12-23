//core
use serenity::framework::standard::{macros::command, Args, CommandResult};
use serenity::model::prelude::*;
use serenity::prelude::*;

//factorial
use num::{One};
use num::bigint::BigUint;

#[command]
pub async fn factorial(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
	fn fac(num: usize) -> BigUint {
		match num {
			0 => BigUint::one(),
			1 => BigUint::one(),
			_ => fac(num - 1) * num,
		}
	}

	let number = args.single::<usize>()?;
	
	msg.channel_id.say(&ctx.http, fac(number)).await?;

	Ok(())
}