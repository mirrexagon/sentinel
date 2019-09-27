use serenity::{
    client::bridge::gateway::{ShardId, ShardManager},
    framework::standard::{
        help_commands,
        macros::{check, command, group, help},
        ArgError, Args, CheckResult, CommandGroup, CommandOptions, CommandResult, DispatchError,
        HelpOptions, StandardFramework, WithWhiteSpace,
    },
    model::{
        channel::{Channel, Message},
        gateway::Ready,
        id::UserId,
        prelude::*,
    },
    prelude::*,
    utils::{content_safe, ContentSafeOptions},
};
use std::{
    collections::{HashMap, HashSet},
    env,
    fmt::Write,
    sync::Arc,
    thread, time,
};

use log::{error, info};

struct ShardManagerContainer;

impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

struct TalkLikeData;

impl TypeMapKey for TalkLikeData {
    type Value = HashMap<UserId, lmarkov::Chain>;
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
        let mut data = ctx.data.write();

        let talk_like_data = data
            .get_mut::<TalkLikeData>()
            .expect("Expected TalkLikeData in ShareMap.");

        let entry = talk_like_data
            .entry(msg.author.id)
            .or_insert(lmarkov::Chain::new(1));

        entry.train(&msg.content);
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
        data.insert::<TalkLikeData>(HashMap::default());
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
                c.on_mention(Some(bot_id))
                    .prefix(".")
                    .delimiter(" ")
                    .owners(owners)
            })
            // Similar to `before`, except will be called directly _after_
            // command execution.
            .after(|_, _, command_name, error| match error {
                Ok(()) => println!("Processed command '{}'", command_name),
                Err(why) => println!("Command '{}' returned error {:?}", command_name, why),
            })
            // Set a function that's called whenever an attempted command-call's
            // command could not be found.
            .unrecognised_command(|_, _, unknown_command_name| {
                println!("Could not find command named '{}'", unknown_command_name);
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
        println!("Client error: {:?}", why);
    }
}

group!({
    name: "general",
    options: {},
    commands: [talklike, quit]
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

#[command]
fn talklike(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    talk_like_wrapper(ctx, msg, args, false)
}

#[command]
fn speaklike(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    talk_like_wrapper(ctx, msg, args, true)
}

const MAX_GENERATE_MESSAGES: usize = 10;
const MAX_GENERATE_TRIES: usize = 10;

fn talk_like_wrapper(ctx: &mut Context, msg: &Message, mut args: Args, tts: bool) -> CommandResult {
    // The first argument should be a user mention or "me".
    let user_id = match args.single::<UserId>() {
        Ok(user_id) => user_id,
        Err(_) => match args.single::<String>() {
            Ok(s) => {
                if s == "me" {
                    msg.author.id
                } else {
                    msg.channel_id.say(
                        &ctx,
                        "I didn't understand. Try `talk like me` or `talk like @user`.",
                    )?;
                    return Ok(());
                }
            }
            Err(err) => {
                error!("Error parsing args: {}", err);
                msg.channel_id
                    .say(&ctx, "An error occurred. Maybe try again?")?;
                return Ok(());
            }
        },
    };

    let num_messages = match args.single::<usize>() {
        Ok(n @ 1..=MAX_GENERATE_MESSAGES) => n,
        Ok(0) => {
            msg.channel_id.say(&ctx, "Generating zero messages.")?;
            return Ok(());
        }
        Ok(_) => {
            msg.channel_id.say(
                &ctx,
                &format!(
                    "I won't generate more than {} messages.",
                    MAX_GENERATE_MESSAGES
                ),
            )?;
            return Ok(());
        }
        Err(err) => match err {
            ArgError::Eos => 1,
            _ => {
                error!("Error parsing args: {}", err);
                msg.channel_id
                    .say(&ctx, "An error occurred. Maybe try again?")?;
                return Ok(());
            }
        },
    };

    // ---

    let mut returned_messages = Vec::new();

    {
        let data = ctx.data.read();
        let talk_like_data = data.get::<TalkLikeData>().unwrap();

        if talk_like_data.contains_key(&user_id) {
            let user_chain = talk_like_data.get(&user_id).unwrap();

            for _ in 0..num_messages {
                let mut text = None;
                for _ in 0..MAX_GENERATE_TRIES {
                    let gen = user_chain.generate();

                    let num_bytes = gen.len();
                    let num_chars = gen.chars().count();
                    info!(
                        "Generated message is {} bytes, {} chars",
                        num_bytes, num_chars
                    );

                    if num_chars > 0 && num_chars < 2000 {
                        text = Some(gen);
                        break;
                    }
                }

                if let Some(text) = text {
                    returned_messages.push(text);
                } else {
                    returned_messages.push(format!("I couldn't generate a message greater than 0 characters or less than 2000 characters (Discord's message size limit)."));
                }
            }
        } else {
            returned_messages.push(format!(
                "Sorry, I don't have a record of {} saying anything.",
                if user_id == msg.author.id {
                    "you".to_owned()
                } else {
                    user_id.mention()
                }
            ));
        }
    };

    for s in returned_messages {
        msg.channel_id
            .send_message(&ctx, |m| m.content(&s).tts(tts))?;

        let msg_delay = time::Duration::from_millis(500);
        thread::sleep(msg_delay);
    }

    Ok(())
}
