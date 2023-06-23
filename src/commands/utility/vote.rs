use crate::{Context, Error};

/// Vote for something
///
/// Enter `~vote Döner` to vote for Döner
#[poise::command(prefix_command, category = "Utility", slash_command)]
pub async fn vote(
    ctx: Context<'_>,
    #[description = "What to vote for"] choice: String,
) -> Result<(), Error> {
    // Lock the Mutex in a block {} so the Mutex isn't locked across an await point
    let num_votes = {
        let mut hash_map = ctx.data().votes.lock().unwrap();
        let num_votes = hash_map.entry(choice.clone()).or_default();
        *num_votes += 1;
        *num_votes
    };

    let response = format!("Successfully voted for {choice}.\n{choice} now has {num_votes} votes!");
    ctx.say(response).await?;
    Ok(())
}

/// Retrieve number of votes
///
/// Retrieve the number of votes either in general, or for a specific choice:
/// ```
/// ~votes
/// ```
///
/// Retrieve the number of votes for a specific choice
/// ```
/// ~votes Döner
/// ```
#[poise::command(prefix_command, track_edits, category = "Utility", aliases("votes"), slash_command)]
pub async fn get_votes(
    ctx: Context<'_>,
    #[description = "Choice to retrieve votes for"] choice: Option<String>,
) -> Result<(), Error> {
    if let Some(choice) = choice {
        let num_votes = *ctx.data().votes.lock().unwrap().get(&choice).unwrap_or(&0);
        let response = match num_votes {
            0 => format!("Nobody has voted for {} yet", choice),
            _ => format!("{} people have voted for {}", num_votes, choice),
        };
        ctx.say(response).await?;
    } else {
        let mut response = String::new();
        for (choice, num_votes) in ctx.data().votes.lock().unwrap().iter() {
            response += &format!("{}: {} votes\n", choice, num_votes);
        }

        if response.is_empty() {
            response += "Nobody has voted for anything yet :(";
        }

        ctx.say(response).await?;
    };

    Ok(())
}

/// Reset one specific or all votes
///
/// Resets all votes.
/// ```
/// reset_votes all
/// ```
/// Resets the vote count for a specific choice.
/// ```
/// reset_votes döner
/// ```
#[poise::command(prefix_command, category = "Utility", slash_command)]
pub async fn reset_votes(
    ctx: Context<'_>,
    #[description = "What to vote for"] choice: String,
) -> Result<(), Error> {
    // Lock the Mutex in a block {} so the Mutex isn't locked across an await point
    let response = {
        let mut hash_map = ctx.data().votes.lock().unwrap();
        let removed = hash_map.remove_entry(&choice);
        if choice == "all" {
            hash_map.clear();
            "Successfully reset all votes".to_string()
        } else {
            match removed {
                None => { format!("No entry found for {}", choice) }
                Some(_) => { format!("Successfully reset the votes for {}", choice) }
            }
        }
    };

    ctx.say(response).await?;
    Ok(())
}