mod talk_like;

use talk_like::{CLEAR_COMMAND, MIMICTTS_COMMAND, MIMIC_COMMAND};

use serenity::{
    client::bridge::gateway::ShardManager,
    framework::standard::{
        macros::{command, group},
        CommandResult, DispatchError, StandardFramework,
    },
    model::{channel::Message, gateway::Ready, prelude::*},
    prelude::*,
};

use std::{
    collections::{HashMap, HashSet},
    env,
    sync::Arc,
};

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

const COMMAND_PREFIX: &str = ".";

struct ShardManagerContainer;
impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

struct Handler;
impl EventHandler for Handler {
    fn ready(&self, _: Context, ready: Ready) {
        info!("Connected as {}", ready.user.name);
    }

    fn resume(&self, _: Context, _: ResumedEvent) {
        info!("Resumed");
    }

    fn message(&self, ctx: Context, msg: Message) {
        talk_like::on_message(&ctx, &msg);
    }
}

fn main() {
    kankyo::load().expect("Failed to load .env file");

    env_logger::init();

    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let mut client = Client::new(&token, Handler).expect("Err creating client");

    {
        let mut data = client.data.write();

        match talk_like::load() {
            Ok(talk_like_data) => {
                info!("Successfully loaded talk like data");
                data.insert::<talk_like::Key>(talk_like_data);
            }
            Err(err) => {
                error!("Error loading talk like data: {:?}", err);
                info!("Using empty talk like data");
                data.insert::<talk_like::Key>(HashMap::new());
            }
        }

        data.insert::<ShardManagerContainer>(Arc::clone(&client.shard_manager));
    }

    // We will fetch your bot's owners and id
    let (owners, bot_id) = match client.cache_and_http.http.get_current_application_info() {
        Ok(info) => {
            let mut owners = HashSet::new();
            owners.insert(info.owner.id);

            (owners, info.id)
        }
        Err(why) => panic!("Could not access application info: {:?}", why),
    };

    client.with_framework(
        // Configures the client, allowing for options to mutate how the
        // framework functions.
        //
        // Refer to the documentation for
        // `serenity::ext::framework::Configuration` for all available
        // configurations.
        StandardFramework::new()
            .configure(|c| {
                c.owners(owners)
                    .with_whitespace(false)
                    .prefix(COMMAND_PREFIX)
                    .on_mention(Some(bot_id))
            })
            // Similar to `before`, except will be called directly _after_
            // command execution.
            .after(|_, _, command_name, error| match error {
                Ok(()) => info!("Processed command '{}'", command_name),
                Err(why) => info!("Command '{}' returned error {:?}", command_name, why),
            })
            // Set a function that's called whenever an attempted command-call's
            // command could not be found.
            .unrecognised_command(|_, _, unknown_command_name| {
                info!("Could not find command named '{}'", unknown_command_name);
            })
            // Set a function that's called whenever a command's execution didn't complete for one
            // reason or another. For example, when a user has exceeded a rate-limit or a command
            // can only be performed by the bot owner.
            .on_dispatch_error(|ctx, msg, error| {
                if let DispatchError::Ratelimited(seconds) = error {
                    let _ = msg.channel_id.say(
                        &ctx.http,
                        &format!("Try this again in {} seconds.", seconds),
                    );
                }
            })
            .group(&GENERAL_GROUP),
    );

    if let Err(why) = client.start() {
        error!("Client error: {:?}", why);
    }
}

group!({
    name: "general",
    options: {},
    commands: [mimic, mimictts, clear, quit]
});

#[command]
#[owners_only]
fn quit(ctx: &mut Context, msg: &Message) -> CommandResult {
    let data = ctx.data.read();

    if let Some(manager) = data.get::<ShardManagerContainer>() {
        manager.lock().shutdown_all();
    } else {
        let _ = msg.reply(&ctx, "There was a problem getting the shard manager");

        return Ok(());
    }

    let _ = msg.reply(&ctx, "Shutting down!");

    Ok(())
}
