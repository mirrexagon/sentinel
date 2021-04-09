use std::{collections::{HashMap, HashSet}, env, fmt::Write, sync::Arc, io, fs::File};
use serenity::{
    async_trait,
    client::bridge::gateway::{ShardId, ShardManager},
    framework::standard::{
        Args, CommandOptions, CommandResult, CommandGroup,
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

use lmarkov::Chain;

const CHAIN_ORDER_DEFAULT: usize = 1;
const CHAIN_DATA_FILE_PATH: &'static str = "chains.json";

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
    kankyo::init().unwrap();

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
                   .delimiters(vec![", ", ","])
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

// Repeats what the user passed as argument but ensures that user and role
// mentions are replaced with a safe textual alternative.
// In this example channel mentions are excluded via the `ContentSafeOptions`.
#[command]
#[bucket = "talk_like"]
async fn talk_like(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    match generate_text(ctx, msg.author.id, None).await {
        Some(text) => msg.reply(ctx, text).await?,
        None => msg.reply(ctx, "Could not generate message").await?,
    };

    Ok(())
}

// Repeats what the user passed as argument but ensures that user and role
// mentions are replaced with a safe textual alternative.
// In this example channel mentions are excluded via the `ContentSafeOptions`.
#[command]
#[bucket = "talk_like"]
async fn speak_like(ctx: &Context, msg: &Message, args: Args) -> CommandResult {


    Ok(())
}

async fn generate_text(ctx: &Context, user_id: UserId, seed_word: Option<&str>) -> Option<String> {
    let data = ctx.data.read().await;
    let chains = data.get::<MarkovChainContainerKey>().expect("Expected MarkovChainContainerKey in TypeMap.");

    let user_chain = chains.by_user.get(&user_id)?;

    // TODO: Use seed word(s)
    Some(user_chain.generate())
}

// Repeats what the user passed as argument but ensures that user and role
// mentions are replaced with a safe textual alternative.
// In this example channel mentions are excluded via the `ContentSafeOptions`.
#[command]
#[bucket = "talk_like"]
async fn clear_my_data(ctx: &Context, msg: &Message, args: Args) -> CommandResult {


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
