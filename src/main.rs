// TODO:
// - Markov
// - Some kind of auth and permissions
// - Voice commands

// -- Crates --
#[macro_use] extern crate serenity;
extern crate typemap;
extern crate markov;

// -- Modules --
mod meta;
mod talklike;

// -- Use --
use std::env;

use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::framework::standard::{Args, DispatchError, StandardFramework, HelpBehaviour, CommandOptions, help_commands};
use serenity::prelude::*;

// -- Constants --
const DATA_DIRECTORY: &'static str = "./data";

// -- Handler --
struct Handler;

impl EventHandler for Handler {
    fn message(&self, ctx: Context, msg: Message) {
        talklike::on_message(&ctx, &msg);
    }

    fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

// -- Main --
fn main() {
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let mut client = Client::new(&token, Handler).expect("Err creating client");

    client.with_framework(
        StandardFramework::new()
        .configure(|c| c
            .allow_whitespace(false)
            .on_mention(true)
            .prefix(".")
            .delimiter(" "))

        .after(|_, _, command_name, error| {
            match error {
                Ok(()) => println!("Processed command '{}'", command_name),
                Err(why) => println!("Command '{}' returned error {:?}", command_name, why),
            }
        })

        .unrecognised_command(|_, _, unknown_command_name| {
            println!("Could not find command named '{}'", unknown_command_name);
        })

        .on_dispatch_error(|_ctx, msg, error| {
            if let DispatchError::RateLimited(seconds) = error {
                let _ = msg.channel_id.say(&format!("Try this again in {} seconds.", seconds));
            }
        })

        .help(help_commands::with_embeds)
        .command("save", |c| c
                .cmd(meta::commands::save))
        .command("quit", |c| c
                .cmd(meta::commands::quit))                
        .command("talklikeme", |c| c
                .cmd(talklike::commands::talklikeme)
         )
    );

    talklike::init(&mut client);

    if let Err(why) = client.start() {
        println!("Client error: {:?}", why);
    }
}
