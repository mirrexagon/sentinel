// TODO:
// - Voice commands
// - Do error propogation properly in the modules
// - Make a more automagic module system

// -- Crates --
#[macro_use]
extern crate log;
extern crate env_logger;

#[macro_use]
extern crate serenity;

extern crate markov;
extern crate typemap;

// -- Modules --
mod modules;

use modules::talklike;

// -- Use --
use std::collections::HashSet;
use std::env;
use std::path::Path;
use std::process;

use serenity::framework::standard::{
    help_commands, Args, CommandOptions, DispatchError, HelpBehaviour, StandardFramework,
};
use serenity::http;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

// -- Type aliases --
type SerenityResult<T> = Result<T, SerenityError>;

// -- Handler --
struct Handler;

impl EventHandler for Handler {
    fn message(&self, ctx: Context, msg: Message) {
        talklike::on_message(&ctx, &msg);
    }

    fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }
}

// -- Main --
fn main() {
    env_logger::init();

    // ---

    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let mut client = Client::new(&token, Handler).expect("Err creating client");

    // Get hashset of owners for the framework.
    let owners = match http::get_current_application_info() {
        Ok(info) => {
            let mut set = HashSet::new();
            set.insert(info.owner.id);

            set
        }
        Err(why) => panic!("Couldn't get application info: {:?}", why),
    };

    // Setup the framework.
    let mut framework = StandardFramework::new()
        .configure(|c| {
            c.allow_whitespace(false)
                .on_mention(true)
                .prefix(".")
                .delimiter(" ")
                .owners(owners)
        })
        .after(|_, _, command_name, error| match error {
            Ok(()) => info!("Processed command '{}'", command_name),
            Err(why) => error!("Command '{}' returned error {:?}", command_name, why),
        })
        .unrecognised_command(|_, _, unknown_command_name| {
            info!("Could not find command named '{}'", unknown_command_name);
        })
        .on_dispatch_error(|_ctx, msg, error| {
            if let DispatchError::RateLimited(seconds) = error {
                let _ = msg
                    .channel_id
                    .say(&format!("Try this again in {} seconds.", seconds));
            }
        })
        .help(help_commands::with_embeds)
        .simple_bucket("save", 5)
        .command("save", |c| c.cmd(cmd_save).bucket("save"))
        .command("quit", |c| c.cmd(cmd_quit).owners_only(true));

    // Init the modules.
    framework = talklike::init_framework(framework);
    talklike::init_client(&mut client, Path::new("data/talklike"))
        .expect("Failed to init talklike");

    // Start!
    client.with_framework(framework);
    if let Err(why) = client.start() {
        error!("Client error: {:?}", why);
    }
}

// -- Base commands --
fn save_data(ctx: &Context) {
    info!("Saving all modules' data");
    if let Err(err) = talklike::save_data(ctx, Path::new("data/talklike")) {
        error!("Error saving talklike data: {:?}", err);
    }
}

command!(
    cmd_save(ctx, msg, _args) {
        save_data(&ctx);
        if let Err(err) = msg.channel_id.say("Saved!") {
            error!("Error sending saved message: {:?}", err);
        }
    }
);

command!(
    cmd_quit(ctx, msg, _args) {
        save_data(&ctx);

        if let Err(err) = msg.channel_id.say("Bye!") {
            error!("Error saying bye: {:?}", err);
        }

        ctx.shard.shutdown_clean();
        
        process::exit(0);
    }
);
