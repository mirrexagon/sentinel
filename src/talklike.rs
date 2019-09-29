use std::io::{Read, Write};
use std::{collections::HashMap, fs::File, thread, time};

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use serenity::framework::standard::{macros::command, ArgError, Args, CommandResult};
use serenity::model::channel::Message;
use serenity::model::prelude::*;
use serenity::prelude::*;

const MARKOV_CHAIN_ORDER: usize = 2;
const MAX_GENERATE_MESSAGES: usize = 5;
const MAX_GENERATE_TRIES: usize = 10;

pub type TalkLikeMap = HashMap<UserId, lmarkov::Chain>;

pub struct Key;
impl TypeMapKey for Key {
    type Value = TalkLikeMap;
}

pub fn save(map: &TalkLikeMap) -> std::io::Result<()> {
    let json = serde_json::to_string(map)?;
    let mut file = File::create("./talklike.json")?;

    debug!("Writing: {}", json);
    write!(&mut file, "{}", &json)?;

    Ok(())
}

pub fn load() -> std::io::Result<TalkLikeMap> {
    let mut file = File::open("./talklike.json")?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    serde_json::from_str(&contents)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{}", err)))
}

pub fn on_message(ctx: &Context, msg: &Message) {
    if should_process_message(&ctx, &msg) {
        debug!(
            "Processing message '{}' from {}",
            msg.content, msg.author.id
        );

        let mut data = ctx.data.write();
        let talk_like_data = data.get_mut::<Key>().unwrap();

        let entry = talk_like_data
            .entry(msg.author.id)
            .or_insert(lmarkov::Chain::new(MARKOV_CHAIN_ORDER));

        entry.train(&msg.content);

        debug!("Hihi");

        if let Err(err) = save(talk_like_data) {
            error!("Error saving data: {:?}", err);

            let _ = msg
                .channel_id
                .say(&ctx, format!("Could not save talk data: {:?}", err));
        } else {
            debug!("Saved data successfully");
        }
    }
}

fn should_process_message(ctx: &Context, msg: &Message) -> bool {
    // TODO: Figure out properly whether the message is a command.
    debug!("Checking message: {}", msg.content);
    let allowed = !(msg.content.starts_with(".")
        || msg.content.starts_with(&ctx.cache.read().user.mention()));

    debug!("Message allowed: {}", allowed);
    allowed
}

#[command]
fn clear(ctx: &mut Context, msg: &Message, _args: Args) -> CommandResult {
    {
        let mut data = ctx.data.write();
        let talk_like_data = data.get_mut::<Key>().unwrap();

        // Replace with a blank chain.
        // It was either this or remove it and delete the file.
        talk_like_data.insert(msg.author.id, lmarkov::Chain::new(MARKOV_CHAIN_ORDER));

        save(talk_like_data)?;
    }

    msg.channel_id
        .say(ctx, "Your talking database has been cleared.")?;

    Ok(())
}

#[command]
fn talk(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    talk_like_wrapper(ctx, msg, args, false)
}

#[command]
fn speak(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    talk_like_wrapper(ctx, msg, args, true)
}

fn talk_like_wrapper(ctx: &mut Context, msg: &Message, mut args: Args, tts: bool) -> CommandResult {
    info!("Talk like wrapper");

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

    let mut returned_messages = Vec::new();

    {
        let data = ctx.data.read();
        let talk_like_data = data.get::<Key>().unwrap();

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
