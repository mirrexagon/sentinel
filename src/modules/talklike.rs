// Ideas:
// - Save periodically. Do this for all modules in main?
// - Disarm @everyone, and maybe other mentions too.
// - Maybe add a command to read all of a user's message history in a channel.
// - Make it per user per server? Users would expect it to keep
// what they say per server.

// -- Use --
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serenity::framework::standard::StandardFramework;
use serenity::model::prelude::*;
use serenity::prelude::*;
use serenity::CACHE;
use SerenityResult;

use markov;
use typemap;

// -- Constants --
const MARKOV_ORDER: usize = 1;
const MAX_GENERATE_TRIES: usize = 10;
const MAX_GENERATE_MESSAGES: usize = 5;

// -- Data --
pub struct Data {
    by_user: HashMap<UserId, markov::Chain<String>>,
    data_dir: PathBuf,
}

impl Data {
    pub fn new() -> Self {
        Data {
            by_user: HashMap::new(),
            data_dir: PathBuf::new(), // TODO: Don't store this.
        }
    }
}

// -- Helpers --
/// Checks that a message successfully sent; if not, then logs why.
fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        error!("Error sending message: {:?}", why);
    }
}

// -- Module --
pub struct TalkLike;

impl typemap::Key for TalkLike {
    type Value = Data;
}

pub fn init_client(client: &mut Client, data_dir: &Path) -> SerenityResult<()> {
    let mut module_data = Data::new();
    module_data.data_dir = PathBuf::from(data_dir);

    if data_dir.is_dir() {
        for entry in data_dir.read_dir()? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                info!("Loading {}", path.display());

                let file_stem = path.file_stem();
                if let None = file_stem {
                    error!("Could not get file stem");
                    continue;
                }
                let file_stem = file_stem.unwrap();

                let file_stem_string = file_stem.to_str();
                if let None = file_stem_string {
                    error!("File stem is not valid Unicode");
                    continue;
                }
                let file_stem_string = file_stem_string.unwrap();

                let user_id_num = file_stem_string.parse::<u64>();
                if let Err(err) = user_id_num {
                    error!("Could not parse file stem into an integer: {:?}", err);
                    continue;
                }

                let user_id = UserId(user_id_num.unwrap());

                match markov::Chain::load(&path) {
                    Ok(chain) => {
                        module_data.by_user.insert(user_id, chain);
                    }
                    Err(err) => {
                        error!("Failed to load {}: {:?}", path.display(), err);
                    }
                }
            }
        }
    }

    {
        let mut client_data = client.data.lock();
        client_data.insert::<TalkLike>(module_data);
    }

    Ok(())
}

pub fn init_framework(framework: StandardFramework) -> StandardFramework {
    framework
        .simple_bucket("talklikegen", 5)
        .command("talk like", |c| {
            c.cmd(commands::talk_like).bucket("talklikegen")
            .desc("Have the bot generate some text based on what a user has said. Try `talk like me` and `talk like @user`.")
        })
        .command("speak like", |c| {
            c.cmd(commands::speak_like).bucket("talklikegen")
            .desc("Have the bot generate some text based on what a user has said, and TTS it. Try `talk like me` and `talk like @user`.")
        })        .command("clear my talk data", |c| {
            c.cmd(commands::clear_my_talk_data)
            .desc("Clears the data used to generate text when `talk like` is used on you.")
        })
}

pub fn save_data(ctx: &Context, data_dir: &Path) -> SerenityResult<()> {
    let mut client_data = ctx.data.lock();
    let module_data = client_data.get_mut::<TalkLike>().unwrap();

    let mut path = PathBuf::from(data_dir);
    fs::create_dir_all(&path)?;

    for (user_id, chain) in &module_data.by_user {
        path.push(format!("{}", user_id));
        path.set_extension("chain");

        match chain.save(&path) {
            Ok(()) => info!("Saved {}", path.display()),
            Err(e) => error!("Failed to save {}: {:?}", path.display(), e),
        }

        path.pop();
    }

    Ok(())
}

fn should_process_message(msg: &Message) -> bool {
    // TODO: Figure out properly whether the message is a command.
    !(msg.author.id == CACHE.read().user.id) && !msg.content.starts_with(".") && !msg.content.starts_with(&CACHE.read().user.mention())
}

pub fn on_message(ctx: &Context, msg: &Message) {
    if should_process_message(msg) {
        let mut client_data = ctx.data.lock();
        let module_data = client_data.get_mut::<TalkLike>().unwrap();

        let chain = module_data
            .by_user
            .entry(msg.author.id)
            .or_insert(markov::Chain::of_order(MARKOV_ORDER));
        chain.feed_str(&msg.content);
    } else {
        info!("talklike not processing message: {}", msg.content);
    }
}

// -- Commands --
mod commands {
    use std::thread;
    use std::time;

    use ::SerenityResult;
    use super::{check_msg, TalkLike, MARKOV_ORDER, MAX_GENERATE_TRIES, MAX_GENERATE_MESSAGES};
    use serenity::prelude::*;
    use serenity::model::prelude::*;
    use serenity::framework::standard::{Args, ArgError};
    use markov;


    fn talk_like_wrapper(ctx: &mut Context, msg: &Message, mut args: Args, tts: bool) -> SerenityResult<()> {
        // The first argument should be a user mention or "me".
        let user_id = match args.single_n::<UserId>() {
            Ok(user_id) => user_id,
            Err(_) => {
                match args.single_n::<String>() {
                    Ok(s) => if s == "me" { msg.author.id } else {
                        check_msg(msg.channel_id.say("I didn't understand. Try `talk like me` or `talk like @user`."));
                        return Ok(());
                    },
                    Err(err) => {
                        error!("Error parsing args: {}", err);
                        check_msg(msg.channel_id.say("An error occurred. Maybe try again?"));
                        return Ok(());
                    }
                }
            }
        };

        args.skip();

        let num_messages = match args.single::<usize>() {
            Ok(n @ 1 ... MAX_GENERATE_MESSAGES) => n,
            Ok(0) => {
                check_msg(msg.channel_id.say("Generating zero messages."));
                return Ok(());
            }
            Ok(_) => {
                check_msg(msg.channel_id.say(&format!("I won't generate more than {} messages.", MAX_GENERATE_MESSAGES)));
                return Ok(());
            }
            Err(err) => {
                match err {
                    ArgError::Eos => 1,
                    _ => {
                        error!("Error parsing args: {}", err);
                        check_msg(msg.channel_id.say("An error occurred. Maybe try again?"));
                        return Ok(());
                    }
                }
            }
        };

        // ---

        let mut returned_messages = Vec::new();

        {
            let mut data = ctx.data.lock();
            let module_data = data.get_mut::<TalkLike>().unwrap();

            if module_data.by_user.contains_key(&user_id) {
                let user_chain = module_data.by_user.get(&user_id).unwrap();

                for _ in 0..num_messages {
                    let mut text = None;
                    for _ in 0..MAX_GENERATE_TRIES {
                        let gen = user_chain.generate_str();
                        info!("Generated message is {} bytes, {} chars", gen.len(), gen.chars().count());
                        if gen.chars().count() < 2000 {
                            text = Some(gen);
                            break;
                        }
                    }

                    if let Some(text) = text {
                        returned_messages.push(text);
                    } else {
                        returned_messages.push(format!("I couldn't generate a message less than 2000 characters (Discord's message size limit)."));
                    }
                }
            } else {
                returned_messages.push(format!("Sorry, I don't have a record of {} saying anything.",
                       if user_id == msg.author.id { "you".to_owned() }
                       else { user_id.mention() }));
            }
        };

        for s in returned_messages {
            check_msg(msg.channel_id.send_message(|m| m.content(&s).tts(tts)));

            let msg_delay = time::Duration::from_millis(500);
            thread::sleep(msg_delay);
        }

        Ok(())
    }

    command!(
        clear_my_talk_data(ctx, msg, _args) {
            let data_dir;
            {
                let mut data = ctx.data.lock();
                let mut module_data = data.get_mut::<TalkLike>().unwrap();

                // Replace with a blank chain.
                // It was either this or remove it and delete the file.
                module_data.by_user.insert(msg.author.id, markov::Chain::of_order(MARKOV_ORDER));
                data_dir = module_data.data_dir.clone();
            }

            super::save_data(&ctx, &data_dir)
                .map_err(|err| error!("Error saving talklike data after clear: {:?}", err));

            check_msg(msg.channel_id.say("Your talking database has been cleared."));

        }
    );

    command!(
        talk_like(ctx, msg, args) {
            talk_like_wrapper(ctx, msg, args, false);
        }
    );

    command!(
        speak_like(ctx, msg, args) {
            talk_like_wrapper(ctx, msg, args, true);
        }
    );
}
