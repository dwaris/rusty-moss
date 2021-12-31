#![allow(non_snake_case)]

use serenity::framework::standard::{macros::command, Args, CommandResult};
use serenity::model::prelude::*;
use serenity::prelude::*;

use serde::Deserialize;

#[command]
pub async fn covid(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    #[derive(Deserialize, Debug)]
    struct Corona {
        delta: Delta,
        weekIncidence: f32,
        hospitalization: Hospital,
        r: RValue,
        meta: Meta,
    }

    #[derive(Deserialize, Debug)]
    struct Delta {
        cases: u32,
        deaths: u16,
    }

    #[derive(Deserialize, Debug)]
    struct RValue {
        value: f32,
    }

    #[derive(Deserialize, Debug)]
    struct Hospital {
        incidence7Days: f32,
    }
    #[derive(Deserialize, Debug)]
    struct Meta {
        lastUpdate: String,
    }

    fn cases(message: &mut String, response: &Corona) {
        let value = response.delta.cases;
        message.push_str(&format!("**New Infections:** {}", value));
        if value < 10000 {
            message.push_str("  :blush:");
        } else if value < 20000 {
            message.push_str("  :cry:");
        } else if value < 40000 {
            message.push_str("  :angry:");
        } else {
            message.push_str("  :face_with_symbols_over_mouth:");
        }
    }

    fn deaths(message: &mut String, response: &Corona) {
        let value = response.delta.deaths;
        message.push_str(&format!("**New Deaths:** {}", value));
        if value < 10 {
            message.push_str("  :blush:");
        } else if value < 50 {
            message.push_str("  :cry:");
        } else if value < 100 {
            message.push_str("  :angry:");
        } else {
            message.push_str("  :face_with_symbols_over_mouth:");
        }
    }

    fn week_incidence(message: &mut String, response: &Corona) {
        let value = response.weekIncidence;
        message.push_str(&format!("**Weekly Incidence:** {}", value));
        if value < 30.0 {
            message.push_str("  :blush:");
        } else if value < 70.0 {
            message.push_str("  :cry:");
        } else if value < 100.0 {
            message.push_str("  :angry:");
        } else {
            message.push_str("  :face_with_symbols_over_mouth:");
        }
    }

    fn incidence_7days(message: &mut String, response: &Corona) {
        let value = response.hospitalization.incidence7Days;
        message.push_str(&format!("**Hospital Incidence:** {}", value));
        if value < 3.0 {
            message.push_str("  :blush:");
        } else if value < 6.0 {
            message.push_str("  :cry:");
        } else if value < 9.0 {
            message.push_str("  :angry:");
        } else {
            message.push_str("  :face_with_symbols_over_mouth:");
        }
    }

    fn r_value(message: &mut String, response: &Corona) {
        let value = response.r.value;
        message.push_str(&format!("**R-Value:** {}", value));
        if value < 1.0 {
            message.push_str("  :blush:");
        } else if value < 2.0 {
            message.push_str("  :angry:");
        } else {
            message.push_str("  :face_with_symbols_over_mouth:");
        }
    }

    fn summary(message: &mut String, response: &Corona) {
        message.push_str(&format!(
            "**Last Updated:** {}",
            &response.meta.lastUpdate[0..=9]
        ));
        message.push('\n');
        cases(message, response);
        message.push('\n');
        deaths(message, response);
        message.push('\n');
        week_incidence(message, response);
        message.push('\n');
        incidence_7days(message, response);
        message.push('\n');
        r_value(message, response);
    }

    let value = args.single::<String>()?;
    let mut message = String::new();
    let response = reqwest::get("https://api.corona-zahlen.org/germany/")
        .await?
        .json::<Corona>()
        .await?;

    match value.as_str() {
        "c" => cases(&mut message, &response),
        "d" => deaths(&mut message, &response),
        "i" => week_incidence(&mut message, &response),
        "h" => incidence_7days(&mut message, &response),
        "r" => r_value(&mut message, &response),
        "s" => summary(&mut message, &response),
        _ => message.push_str(&format!(
            "Invalid Request: `{}`! Try again with `c`, `d`, `i`, `h`, `r` or `s`:",
            value
        )),
    };

    msg.channel_id.say(&ctx.http, message).await?;

    Ok(())
}
