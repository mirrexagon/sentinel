use std::{collections::{HashMap, HashSet}, env, fmt::Write, sync::Arc, io, fs::File, time, thread};
use serenity::{
    async_trait,
    client::bridge::gateway::{ShardId, ShardManager},
    framework::standard::{
        Args, ArgError, CommandOptions, CommandResult, CommandGroup,
        DispatchError, HelpOptions, help_commands, Reason, StandardFramework,
        buckets::{RevertBucket, LimitedFor},
        macros::{command, group, help, check, hook},
    },
    http::Http,
    model::{
        channel::{Channel, Message},
        gateway::Ready,
        id::UserId,
        permissions::Permissions,
    },
    utils::{content_safe, ContentSafeOptions},
};

use serenity::prelude::*;
use tokio::sync::Mutex;

use serde::{Serialize, Deserialize};

use lmarkov::{Chain, ChainKey};

const CHAIN_DATA_FILE_PATH: &'static str = "chains.json";
const CHAIN_ORDER_DEFAULT: usize = 1;
const MAX_GENERATE_MESSAGES: usize = 5;
const MAX_GENERATE_TRIES: usize = 10;

struct ShardManagerContainer;
impl TypeMapKey for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

struct MarkovChainContainerKey;

#[derive(Serialize, Deserialize)]
struct MarkovChainContainer {
    by_user: HashMap<UserId, Chain>,
}

impl TypeMapKey for MarkovChainContainerKey {
    type Value = MarkovChainContainer;
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[group]
#[commands(talk_like, speak_like, clear_my_data)]
struct TalkLike;

#[hook]
async fn before(ctx: &Context, msg: &Message, command_name: &str) -> bool {
    println!("Got command '{}' by user '{}'", command_name, msg.author.name);
    true
}

#[hook]
async fn after(_ctx: &Context, _msg: &Message, command_name: &str, command_result: CommandResult) {
    match command_result {
        Ok(()) => println!("Processed command '{}'", command_name),
        Err(why) => println!("Command '{}' returned error {:?}", command_name, why),
    }
}

#[hook]
async fn unknown_command(_ctx: &Context, _msg: &Message, unknown_command_name: &str) {
    println!("Could not find command named '{}'", unknown_command_name);
}

#[hook]
async fn normal_message(ctx: &Context, msg: &Message) {
    println!("Message is not a command '{}'", msg.content);

    let mut data = ctx.data.write().await;
    let chains = data.get_mut::<MarkovChainContainerKey>().expect("Expected MarkovChainContainerKey in TypeMap.");

    {
        let user_chain = chains.by_user.entry(msg.author.id).or_insert(Chain::new(CHAIN_ORDER_DEFAULT));
        user_chain.train(&msg.content);
    }

    match save_chains(chains) {
        Ok(_) => {},
        Err(e) => println!("Error saving chains: {:?}", e),
    }
}

#[hook]
async fn delay_action(ctx: &Context, msg: &Message) {
    // You may want to handle a Discord rate limit if this fails.
    let _ = msg.react(ctx, '⏱').await;
}

#[hook]
async fn dispatch_error(ctx: &Context, msg: &Message, error: DispatchError) {
    if let DispatchError::Ratelimited(info) = error {
        // We notify them only once.
        if info.is_first_try {
            let _ = msg
                .channel_id
                .say(&ctx.http, &format!("Try this again in {} seconds.", info.as_secs()))
                .await;
        }
    }
}

#[tokio::main]
async fn main() {
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect(
        "Expected a token in the environment",
    );

    let http = Http::new_with_token(&token);

    // We will fetch your bot's owners and id
    let (owners, bot_id) = match http.get_current_application_info().await {
        Ok(info) => {
            let mut owners = HashSet::new();
            if let Some(team) = info.team {
                owners.insert(team.owner_user_id);
            } else {
                owners.insert(info.owner.id);
            }
            match http.get_current_user().await {
                Ok(bot_id) => (owners, bot_id.id),
                Err(why) => panic!("Could not access the bot id: {:?}", why),
            }
        },
        Err(why) => panic!("Could not access application info: {:?}", why),
    };

    let framework = StandardFramework::new()
        .configure(|c| c
                   .with_whitespace(true)
                   .on_mention(Some(bot_id))
                   .prefix(".")
                   .delimiters(vec![" "])
                   .owners(owners))
        .before(before)
        .after(after)
        .unrecognised_command(unknown_command)
        .normal_message(normal_message)
        .on_dispatch_error(dispatch_error)
        .bucket("talk_like", |b| b.delay(5)).await
        .group(&TALKLIKE_GROUP);

    let mut client = Client::builder(&token)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Err creating client");

    let chains = match load_chains() {
        Ok(chains) => {
            println!("Successfully loaded chains");
            chains
        },
        Err(e) => {
            println!("Couldn't load chains, using new empty chains: {:?}", e);
            MarkovChainContainer { by_user: HashMap::new() }
        }
    };

    {
        let mut data = client.data.write().await;
        data.insert::<MarkovChainContainerKey>(chains);
        data.insert::<ShardManagerContainer>(Arc::clone(&client.shard_manager));
    }

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}

#[command]
#[bucket = "talk_like"]
#[aliases("t")]
async fn talk_like(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    talk_like_wrapper(ctx, msg, args, false).await
}

#[command]
#[bucket = "talk_like"]
#[aliases("s")]
async fn speak_like(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    talk_like_wrapper(ctx, msg, args, true).await
}

async fn talk_like_wrapper(ctx: &Context, msg: &Message, mut args: Args, tts: bool) -> CommandResult {
    // The first argument should be a user mention or "me".
    let user_id = match args.single::<UserId>() {
        Ok(user_id) => user_id,
        Err(_) => match args.single::<String>() {
            Ok(s) => {
                if s == "me" {
                    msg.author.id
                } else {
                    msg.channel_id.say(
                        ctx,
                        "I didn't understand. Try `.t me` or `.t @user`.",
                    ).await?;
                    return Ok(());
                }
            }
            Err(err) => {
                println!("Error parsing args in talk_like_wrapper(): {}", err);
                msg.channel_id
                    .say(ctx, "An error occurred.").await?;
                return Ok(());
            }
        },
    };

    // Second argument is optional number of messages.
    let num_messages = match args.single::<usize>() {
        Ok(n @ 1..=MAX_GENERATE_MESSAGES) => n,
        Ok(0) => {
            msg.channel_id.say(&ctx, "Generating zero messages.").await?;
            return Ok(());
        }
        Ok(_) => {
            msg.channel_id.say(
                ctx,
                &format!(
                    "I won't generate more than {} messages.",
                    MAX_GENERATE_MESSAGES
                ),
            ).await?;
            return Ok(());
        }
        Err(err) => match err {
            ArgError::Eos => 1,
            _ => {
                println!("Error parsing args in talk_like_wrapper(): {}", err);
                msg.channel_id
                    .say(&ctx, "An error occurred.").await?;
                return Ok(());
            }
        },
    };

    let mut returned_messages = Vec::new();

    {
        let data = ctx.data.read().await;
        let chains = data.get::<MarkovChainContainerKey>().expect("Expected MarkovChainContainerKey in TypeMap.");

        if let Some(user_chain) = chains.by_user.get(&user_id) {
            for _ in 0..num_messages {
                let mut text = None;
                for _ in 0..MAX_GENERATE_TRIES {
                    let gen = user_chain.generate().unwrap();

                    let num_bytes = gen.len();
                    let num_chars = gen.chars().count();
                    println!(
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
                    user_id.mention().to_string()
                }
            ));
        }
    };

    for s in returned_messages {
         let settings = if let Some(guild_id) = msg.guild_id {
           // By default roles, users, and channel mentions are cleaned.
           ContentSafeOptions::default()
                // We do not want to clean channel mentions as they
                // do not ping users.
                .clean_channel(false)
                // If it's a guild channel, we want mentioned users to be displayed
                // as their display name.
                .display_as_member_from(guild_id)
        } else {
            ContentSafeOptions::default()
                .clean_channel(false)
        };

        println!("Before: {}", s);
        let content = content_safe(&ctx.cache, &s, &settings).await;
        println!("After: {}", content);

        msg.channel_id
            .send_message(&ctx, |m| m.content(&content).tts(tts)).await?;

        let msg_delay = time::Duration::from_millis(500);
        thread::sleep(msg_delay);
    }

    Ok(())
}

// Repeats what the user passed as argument but ensures that user and role
// mentions are replaced with a safe textual alternative.
// In this example channel mentions are excluded via the `ContentSafeOptions`.
#[command]
#[bucket = "talk_like"]
async fn clear_my_data(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    // TODO

    Ok(())
}

fn save_chains(chains: &MarkovChainContainer) -> io::Result<()> {
    let f = File::create(CHAIN_DATA_FILE_PATH)?;
    serde_json::to_writer(f, chains)?;
    Ok(())
}

fn load_chains() -> io::Result<MarkovChainContainer> {
    let f = File::open(CHAIN_DATA_FILE_PATH)?;
    let chains = serde_json::from_reader(f)?;
    Ok(chains)
}
