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
    // stolen from example
    match error {
        poise::FrameworkError::Setup { error, .. } => panic!("Failed to start bot: {:?}", error),
        poise::FrameworkError::Command { error, ctx } => {
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

    let token = env::var("DISCORD_TOKEN").expect("No discord token found in environment variables"); //Get discord token from environment variables

    let options = poise::FrameworkOptions {
        // Add all commands to the framework
        commands: vec![
            fun::ping::ping(),
            fun::ping::pong(),
            utility::vote::vote(),
            utility::vote::getvotes(),
            utility::help::help(),
        ],

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
                println!("Got an event in event handler: {:?}", event.name());
                Ok(())
            })
        },
        ..Default::default() //DEBUG
    };

    poise::Framework::builder()
        .token(token)
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?; //Register all commands globally (for all guilds)
                Ok(Data {
                    votes: Mutex::new(HashMap::new()), //Initialize the votes hashmap
                })
            })
        })
        .options(options)
        .intents(serenity::GatewayIntents::non_privileged())
        .run()
        .await
        .unwrap();
}
