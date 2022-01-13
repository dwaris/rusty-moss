#![allow(non_snake_case)]

use num::ToPrimitive;
use serenity::framework::standard::{macros::command, Args, CommandResult};
use serenity::model::prelude::*;
use serenity::prelude::*;

use serde::Deserialize;
use std::collections::HashMap;

#[command]
pub async fn covid(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let arg = args.single::<String>()?;
    let value = args.single::<String>()?;
    let mut message = String::new();
    if arg == "de".to_string() {
        message.push_str(Germany(value).await.unwrap().to_string().as_str());
    } else {
        message.push_str(Landkreis(arg, value).await.unwrap().to_string().as_str());
    }
    msg.channel_id.say(&ctx.http, message).await?;

    Ok(())
}

async fn Germany(value: String) -> Result<String, Box<dyn std::error::Error>> {
    #[derive(Deserialize, Debug)]
    struct Germany {
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
        rValue7Days: RWeekValue,
    }

    #[derive(Deserialize, Debug)]
    struct RWeekValue {
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

    fn cases(message: &mut String, response: &Germany) {
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
        message.push_str("\n");
    }

    fn deaths(message: &mut String, response: &Germany) {
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
        message.push_str("\n");
    }

    fn week_incidence(message: &mut String, response: &Germany) {
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
        message.push_str("\n");
    }

    fn incidence_7days(message: &mut String, response: &Germany) {
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
        message.push_str("\n");
    }

    fn r_value(message: &mut String, response: &Germany) {
        let value = response.r.rValue7Days.value;
        message.push_str(&format!("**R-Value:** {}", value));
        if value < 1.0 {
            message.push_str("  :blush:");
        } else if value < 2.0 {
            message.push_str("  :angry:");
        } else {
            message.push_str("  :face_with_symbols_over_mouth:");
        }
        message.push_str("\n");
    }

    fn summary(message: &mut String, response: &Germany) {
        message.push_str(&format!(
            "**Last Updated:** {}\n",
            &response.meta.lastUpdate[0..=9]
        ));
        cases(message, response);
        deaths(message, response);
        week_incidence(message, response);
        incidence_7days(message, response);
        r_value(message, response);
    }

    let mut message = String::new();
    let response = reqwest::get("https://api.corona-zahlen.org/germany/")
        .await?
        .json::<Germany>()
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
    Ok(message)
}

async fn Landkreis(arg: String, value: String) -> Result<String, Box<dyn std::error::Error>> {
    #[derive(Deserialize, Debug)]
    struct Landkreis {
        data: HashMap<String, AGS>,
        meta: Meta,
    }

    #[derive(Deserialize, Debug)]
    struct AGS {
        name: String,
        state: String,
        weekIncidence: f32,
        delta: Delta,
    }

    #[derive(Deserialize, Debug)]
    struct Delta {
        cases: i32,
        deaths: i16,
    }

    #[derive(Deserialize, Debug)]
    struct Meta {
        lastUpdate: String,
    }

    fn cases(arg: &String, message: &mut String, response: &Landkreis) {
        let value = response.data[arg].delta.cases.to_u32().unwrap();
        message.push_str(&format!("**New Infections:** {}", value));
        message.push_str("\n");
    }

    fn deaths(arg: &String, message: &mut String, response: &Landkreis) {
        let value = response.data[arg].delta.deaths.to_i16().unwrap();
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
        message.push_str("\n");
    }

    fn week_incidence(arg: &String, message: &mut String, response: &Landkreis) {
        let value = response.data[arg].weekIncidence.to_f32().unwrap();
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
        message.push_str("\n");
    }

    fn summary(arg: &String, message: &mut String, response: &Landkreis) {
        message.push_str(&format!("**District:** {}\n", &response.data[arg].name));
        message.push_str(&format!("**State:** {}\n", &response.data[arg].state));
        message.push_str(&format!(
            "**Last Updated:** {}\n",
            &response.meta.lastUpdate[0..=9]
        ));
        cases(arg, message, response);
        deaths(arg, message, response);
        week_incidence(arg, message, response);
    }

    let mut message = String::new();
    let response = reqwest::get("https://api.corona-zahlen.org/districts/")
        .await?
        .json::<Landkreis>()
        .await?;

    match value.as_str() {
        "c" => cases(&arg, &mut message, &response),
        "d" => deaths(&arg, &mut message, &response),
        "i" => week_incidence(&arg, &mut message, &response),
        "s" => summary(&arg, &mut message, &response),
        _ => message.push_str(&format!(
            "Invalid Request: `{}`! Try again with `c`, `d`, `i`, or `s`:",
            value
        )),
    };
    Ok(message)
}
