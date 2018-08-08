// Ideas:
// - Save periodically. Do this for all modules in main?
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

// -- Constant --
const MARKOV_ORDER: usize = 1;

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
        .simple_bucket("talklikegen", 1)
        .command("talk like", |c| {
            c.cmd(commands::talk_like).bucket("talklikegen")
            .desc("Have the bot generate some text based on what a user has said. Try `talk like me` and `talk like @user`.")
        })
        .command("clear my talk data", |c| {
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
    !msg.content.starts_with(".") && !msg.content.starts_with(&CACHE.read().user.mention())
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
    use super::TalkLike;
    use serenity::model::prelude::*;

    command!(
        clear_my_talk_data(ctx, msg, _args) {
            let data_dir;
            {
                let mut data = ctx.data.lock();
                let mut module_data = data.get_mut::<TalkLike>().unwrap();

                // Replace with a blank chain.
                // It was either this or remove it and delete the file.
                module_data.by_user.insert(msg.author.id, markov::Chain::of_order(MARKOV_ORDER))
                data_dir = module_data.data_dir.clone();
            }

            super::save_data(&ctx, &data_dir)
                .map_err(|err| error!("Error saving talklike data after clear: {:?}", err));

            if let Err(_) = msg.channel_id.say("Your talking database has been cleared.") {
                error!("Error sending error reponse to clearinga user's talklike data");
            }
        }
    );

    command!(
        talk_like(ctx, msg, args) {
            let user_id = {
                // The first argument should be a user mention or "me".
                match args.single_n::<UserId>() {
                    Ok(user_id) => user_id,
                    Err(_) => {
                        match args.single_n::<String>() {
                            Ok(s) => if s == "me" { msg.author.id } else {
                                if let Err(_) = msg.channel_id.say(
                                    "I didn't understand. Try `talk like me` or `talk like @user`.") {
                                    error!("Error sending error reponse to `talk like`");
                                }

                                return Ok(());
                            },
                            _ => {
                                if let Err(_) = msg.channel_id.say(
                                    "I didn't understand. Try `talk like me` or `talk like <@mention>`") {
                                    error!("Error sending error reponse to `talk like`");
                                }

                                return Ok(());
                            }
                        }
                    }
                }
            };

            let returned_message = {
                let mut data = ctx.data.lock();
                let mut module_data = data.get_mut::<TalkLike>().unwrap();

                let user_has_chain = module_data.by_user.contains_key(&user_id);
                if user_has_chain {
                    let user_chain = module_data.by_user.get(&user_id).unwrap();
                    user_chain.generate_str()
                } else {
                    // TODO: Replace "you" with other person if user id is not yours.
                    format!("Sorry, I don't have a record of you saying anything.")
                }
            };

            if let Err(why) = msg.channel_id.say(&returned_message) {
                error!("Error sending talklike message: {:?}", why);
            }
        }
    );
}
