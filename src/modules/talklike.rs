// Ideas:
// - Ignore commands.
// - Make it per user per server? Users would expect it to keep
// what they say per server.

// -- Use --
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

use serenity::model::id::UserId;
use serenity::model::channel::Message;
use serenity::framework::standard::StandardFramework;
use serenity::prelude::*;
use SerenityResult;

use typemap;
use markov;

// -- Constant --
const MARKOV_ORDER: usize = 1;

// -- Data --
pub struct Data {
    by_user: HashMap<UserId, markov::Chain<String>>,
}

impl Data {
    pub fn new() -> Self {
        Data {
            by_user: HashMap::new(),
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
                    Ok(chain) => { module_data.by_user.insert(user_id, chain); }
                    Err(err) => { error!("Failed to load {}: {:?}", path.display(), err); }
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
        .command("talklikeme", |c| c
                .cmd(commands::talklikeme)
                .bucket("talklikegen"))
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

pub fn on_message(ctx: &Context, msg: &Message) {
    let mut client_data = ctx.data.lock();
    let module_data = client_data.get_mut::<TalkLike>().unwrap();

    let chain = module_data.by_user.entry(msg.author.id).or_insert(markov::Chain::of_order(MARKOV_ORDER));
    chain.feed_str(&msg.content);
}



// -- Commands --
mod commands {
    use super::TalkLike;

    command!(
        talklikeme(ctx, msg, _args) {
            let user_id = msg.author.id;

            let returned_message = {
                let mut data = ctx.data.lock();
                let mut module_data = data.get_mut::<TalkLike>().unwrap();

                let user_has_chain = module_data.by_user.contains_key(&user_id);
                if user_has_chain {
                    let user_chain = module_data.by_user.get(&user_id).unwrap();
                    user_chain.generate_str()
                } else {
                    format!("Sorry, I don't have a record of you saying anything.")
                }
            };

            if let Err(why) = msg.channel_id.say(&returned_message) {
                println!("Failed to send message: {:?}", why);
            }
        }
    );
}
