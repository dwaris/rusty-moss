mod commands;
mod tasks;
use commands::{fun, utility, warframe};

use dotenvy::dotenv;
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

use std::any::Any;
use std::sync::Arc;

pub struct BotData {
    pub(crate) http_client: reqwest::Client,
    pub(crate) warframe_cache: moka::future::Cache<String, Arc<dyn Any + Send + Sync>>,
    // UserID -> Stop Signal Sender
    pub(crate) active_animations:
        RwLock<HashMap<serenity::UserId, tokio::sync::watch::Sender<bool>>>,
    pub(crate) start_time: Instant,
}

fn build_bot_data() -> Result<BotData, Error> {
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build()?;

    let warframe_cache = moka::future::Cache::builder()
        .max_capacity(200)
        .time_to_live(Duration::from_secs(WARFRAME_CACHE_TTL_SECS))
        .build();

    Ok(BotData {
        http_client,
        warframe_cache,
        active_animations: RwLock::new(HashMap::new()),
        start_time: Instant::now(),
    })
}

fn framework_commands() -> Vec<poise::Command<BotData, Error>> {
    vec![
        fun::pixel::pixel(),
        fun::pixel::stop(),
        utility::help::help(),
        utility::stats::stats(),
        warframe::relic_lookup::lookup(),
        warframe::relic_farming::farm(),
        warframe::cycles::cycle(),
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
                tracing::debug!("Executing command {}...", ctx.command().qualified_name);
            })
        },
        post_command: |ctx| {
            Box::pin(async move {
                tracing::info!("Executed command {}!", ctx.command().qualified_name);
            })
        },
        skip_checks_for_owners: false,
        event_handler: |_ctx, event| {
            Box::pin(async move {
                tracing::trace!(
                    "Got an event in event handler: {:?}",
                    event.snake_case_name()
                );
                Ok(())
            })
        },
        ..Default::default()
    }
}

fn build_framework(
    is_ready: Arc<std::sync::atomic::AtomicBool>,
) -> poise::Framework<BotData, Error> {
    poise::Framework::builder()
        .setup(move |ctx, ready, framework| {
            let is_ready = is_ready.clone();
            Box::pin(async move {
                tracing::info!("Logged in as {}", ready.user.name);
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;

                let data = build_bot_data()?;

                tasks::uptime::start_uptime_loop(ctx.clone(), data.http_client.clone());

                is_ready.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(data)
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
            tracing::error!("Error in command `{}`: {:?}", ctx.command().name, error);
            let _ = ctx.say(format!("An error occurred: {}", error)).await;
        }
        poise::FrameworkError::CooldownHit {
            remaining_cooldown,
            ctx,
            ..
        } => {
            let _ = ctx
                .say(format!(
                    "Please wait {:.1}s before using `{}` again.",
                    remaining_cooldown.as_secs_f32(),
                    ctx.command().name
                ))
                .await;
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                tracing::error!("Error while handling framework error: {}", e);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let args: Vec<String> = env::args().collect();
    if args
        .iter()
        .any(|arg| arg == "--healthcheck" || arg == "healthcheck")
    {
        let port: u16 = env::var("HEALTH_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(tasks::health::DEFAULT_HEALTH_PORT);

        if tasks::health::run_healthcheck_client(port).await {
            std::process::exit(0);
        } else {
            std::process::exit(1);
        }
    }

    let env_filter = match env::var("RUST_LOG") {
        Ok(ref val) if !val.trim().is_empty() => tracing_subscriber::EnvFilter::builder()
            .with_default_directive(tracing_subscriber::filter::LevelFilter::INFO.into())
            .parse_lossy(val),
        _ => tracing_subscriber::EnvFilter::new("info"),
    };

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("Starting rusty-moss v{}...", env!("CARGO_PKG_VERSION"));

    let health_port: u16 = env::var("HEALTH_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(tasks::health::DEFAULT_HEALTH_PORT);

    let is_ready = Arc::new(std::sync::atomic::AtomicBool::new(false));
    tasks::health::start_health_server(health_port, is_ready.clone(), Instant::now());

    let framework = build_framework(is_ready);
    let token = discord_token();

    let client = serenity::ClientBuilder::new(token, discord_intents())
        .framework(framework)
        .await;

    client.unwrap().start().await.unwrap();
}
