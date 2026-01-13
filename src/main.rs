mod commands;
use commands::{fun, utility};

use dotenvy::dotenv;
use env_logger::Env;
use poise::serenity_prelude as serenity;
use std::{collections::HashMap, env, sync::Mutex};

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>; // Custom Context struct to store votes

// Custom Data struct to store votes
pub struct Data {
    votes: Mutex<HashMap<String, u32>>,
}

async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    // This is our custom error handler
    // They are many errors that can occur, so we only handle the ones we want to customize
    // and forward the rest to the default handler
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
    dotenv().ok(); //Load .env file

    env_logger::init_from_env(Env::default().filter_or("RUST_LOG", "info")); //Initialize logger based on RUST_LOG environment variable

    let options = poise::FrameworkOptions {
        // Add all commands to the framework
        commands: vec![
            fun::ping::ping(),
            fun::ping::pong(),
            utility::vote::vote(),
            utility::vote::get_votes(),
            utility::vote::reset_votes(),
            utility::help::help(),
        ],
        prefix_options: poise::PrefixFrameworkOptions {
            prefix: Some("~".into()),
            ..Default::default()
        },

        //DEBUG OUTPUT ON CONSOLE
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
    };

    let framework = poise::Framework::builder()
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                println!("Logged in as {}", _ready.user.name);
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {
                    votes: Mutex::new(HashMap::new()),
                })
            })
        })
        .options(options)
        .build();

    let token: String = env::var("DISCORD_TOKEN").expect("No discord token found in environment variables"); //Get discord token from environment variables
    let intents =
        serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;

    client.unwrap().start().await.unwrap()

}
