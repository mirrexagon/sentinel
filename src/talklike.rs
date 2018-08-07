// Ideas:
// - Make it per user per server? Users would expect it to keep
// what they say per server.

// -- Use --
use std::fs;
use std::path::PathBuf;
use std::collections::HashMap;

use serenity::model::id::UserId;
use serenity::model::channel::Message;
use serenity::prelude::*;

use typemap;
use markov;

// -- Constant --
const MARKOV_ORDER: usize = 2;

// -- Types --
struct MarkovKey;

impl typemap::Key for MarkovKey {
    type Value = MarkovData;
}

struct MarkovData {
    by_user: HashMap<UserId, markov::Chain<String>>,
}

impl MarkovData {
    pub fn new() -> Self {
        MarkovData {
            by_user: HashMap::new(),
        }
    }
}

// -- Module --
pub fn init(client: &mut Client) -> Result<(), SerenityError> {
    let mut data = client.data.lock();

    let mut data_path = PathBuf::from(::DATA_DIRECTORY);
    data_path.push("markov");

    let mut markov_data = MarkovData::new();

    if data_path.is_dir() {
        for entry in fs::read_dir(data_path)? {
            let entry = entry?;
            let path = entry.path();

            println!("Loading {}", path.display());

            let user_id = UserId(path.file_stem().unwrap().to_str().unwrap().parse()?);

            match markov::Chain::load(&path) {
                Ok(chain) => { markov_data.by_user.insert(user_id, chain); },
                Err(e) => println!("Error loading {}: {:?}", path.display(), e),
            }

        }
    }

    data.insert::<MarkovKey>(MarkovData::new());

    Ok(())
}

pub fn save(ctx: &Context) -> Result<(), SerenityError> {
    let mut data = ctx.data.lock();
    let markov_data = data.get_mut::<MarkovKey>().unwrap();

    let mut path = PathBuf::from(::DATA_DIRECTORY);
    path.push("markov");

    fs::create_dir_all(&path)?;

    for (user_id, chain) in &markov_data.by_user {

        path.push(format!("{}", user_id));
        path.set_extension("chain");

        match chain.save(path.as_path()) {
            Ok(()) => println!("Saved {}", path.display()),
            Err(e) => println!("Failed to save {}: {:?}", path.display(), e),
        }

        path.pop();
    }

    Ok(())
}

pub fn on_message(ctx: &Context, msg: &Message) {
    let mut data = ctx.data.lock();
    let markov_data = data.get_mut::<MarkovKey>().unwrap();

    let chain = markov_data.by_user.entry(msg.author.id).or_insert(markov::Chain::of_order(MARKOV_ORDER));
    chain.feed_str(&msg.content);
}

// -- Commands --
pub mod commands {
    use super::MarkovKey;

    command!(
        talklikeme(ctx, msg, _args) {
            let user_id = msg.author.id;

            let returned_message = {
                let mut data = ctx.data.lock();
                let mut markov_data = data.get_mut::<MarkovKey>().unwrap();

                let user_has_chain = markov_data.by_user.contains_key(&user_id);
                if user_has_chain {
                    let user_chain = markov_data.by_user.get(&user_id).unwrap();
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
