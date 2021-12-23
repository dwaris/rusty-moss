mod commands;

use std::{collections::HashSet, env, sync::Arc};

use commands::{math::*, meta::*, owner::*, covid::*};
use serenity::{
	async_trait,
	client::bridge::gateway::ShardManager,
	framework::{standard::macros::group, StandardFramework},
	http::Http,
	model::{
		event::ResumedEvent,
		gateway::Ready,
	},
	prelude::*,
};
use tracing::{error, info};

pub struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
	type Value = Arc<Mutex<ShardManager>>;
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
	async fn ready(&self, _: Context, ready: Ready) {
		info!("{} is connected!", ready.user.name);
	}

	async fn resume(&self, _: Context, _: ResumedEvent) {
		info!("Resumed");
	}
}

#[group]
#[commands(covid, factorial, ping, pong, quit)]
struct General;

#[tokio::main]
async fn main() {
	dotenv::dotenv().expect("Failed to load .env file");

	tracing_subscriber::fmt::init();

	let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

	let http = Http::new_with_token(&token);

	let (owners, _bot_id) = match http.get_current_application_info().await {
		Ok(info) => {
			let mut owners = HashSet::new();
			owners.insert(info.owner.id);

			(owners, info.id)
		}
		Err(why) => panic!("Could not access application info: {:?}", why),
	};

	let framework = StandardFramework::new()
		.configure(|c| c.owners(owners).prefix("~"))
		.group(&GENERAL_GROUP);
	let mut client = Client::builder(&token).framework(framework).await.expect("Err creating client");

	{
		let mut data = client.data.write().await;
		data.insert::<ShardManagerContainer>(Arc::clone(&client.shard_manager));
	}

	let shard_manager = Arc::clone(&client.shard_manager);

	tokio::spawn(async move {
		tokio::signal::ctrl_c()
			.await
			.expect("Error awaiting CTRL+C");
		shard_manager.lock().await.shutdown_all().await;
	});

	if let Err(why) = client.start().await {
		error!("Client error: {:?}", why);
	}
}