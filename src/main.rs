mod commands;
use commands::{fun, utility, warframe};

use dotenvy::dotenv;
use env_logger::Env;
use poise::serenity_prelude as serenity;
use std::collections::HashMap;
use std::env;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, BotData, Error>;
const WARFRAME_CACHE_TTL_SECS: u64 = 120;
const HTTP_TIMEOUT_SECS: u64 = 12;
const DEFAULT_PREFIX: &str = "~";

#[derive(Clone)]
struct CachedPayload {
    fetched_at: Instant,
    payload: serde_json::Value,
}

pub struct BotData {
    http_client: reqwest::Client,
    warframe_cache: RwLock<HashMap<String, CachedPayload>>,
    warframe_cache_ttl: Duration,
    relic_names_cache: RwLock<Option<(Instant, Vec<String>)>>,
}

fn build_bot_data() -> Result<BotData, Error> {
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()?;

    Ok(BotData {
        http_client,
        warframe_cache: RwLock::new(HashMap::new()),
        warframe_cache_ttl: Duration::from_secs(WARFRAME_CACHE_TTL_SECS),
        relic_names_cache: RwLock::new(None),
    })
}

fn framework_commands() -> Vec<poise::Command<BotData, Error>> {
    vec![
        fun::ping::ping(),
        fun::pixel::pixel(),
        utility::help::help(),
        warframe::relic_lookup::lookup(),
        warframe::relic_farming::farm(),
    ]
}

fn build_framework_options() -> poise::FrameworkOptions<BotData, Error> {
    poise::FrameworkOptions {
        commands: framework_commands(),
        prefix_options: poise::PrefixFrameworkOptions {
            prefix: Some(DEFAULT_PREFIX.into()),
            ..Default::default()
        },
        on_error: |error| Box::pin(on_error(error)),
        pre_command: |ctx| {
            Box::pin(async move {
                println!("Executing command {}...", ctx.command().qualified_name);
            })
        },
        post_command: |ctx| {
            Box::pin(async move {
                println!("Executed command {}!", ctx.command().qualified_name);
            })
        },
        skip_checks_for_owners: false,
        event_handler: |_ctx, event, _framework, _data| {
            Box::pin(async move {
                println!(
                    "Got an event in event handler: {:?}",
                    event.snake_case_name()
                );
                Ok(())
            })
        },
        ..Default::default()
    }
}

fn build_framework() -> poise::Framework<BotData, Error> {
    poise::Framework::builder()
        .setup(move |ctx, ready, framework| {
            Box::pin(async move {
                println!("Logged in as {}", ready.user.name);
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                build_bot_data()
            })
        })
        .options(build_framework_options())
        .build()
}

fn discord_intents() -> serenity::GatewayIntents {
    serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT
}

fn discord_token() -> String {
    env::var("DISCORD_TOKEN").expect("No discord token found in environment variables")
}


async fn on_error(error: poise::FrameworkError<'_, BotData, Error>) {
    match error {
        poise::FrameworkError::Setup { error, .. } => panic!("Failed to start bot: {:?}", error),
        poise::FrameworkError::Command { error, ctx, .. } => {
            println!("Error in command `{}`: {:?}", ctx.command().name, error,);
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                println!("Error while handling error: {}", e)
            }
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    env_logger::init_from_env(Env::default().filter_or("RUST_LOG", "info"));

    let framework = build_framework();
    let token = discord_token();

    let client = serenity::ClientBuilder::new(token, discord_intents())
        .framework(framework)
        .await;

    client.unwrap().start().await.unwrap();
}
