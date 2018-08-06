// TODO:
// - Markov
// - Some kind of auth and permissions
// - Voice commands


// -- Crates --
extern crate serenity;


// -- Use --
use std::env;

use serenity::prelude::*;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;


// -- Handler --
struct Handler;

impl EventHandler for Handler {
    fn message(&self, ctx: Context, msg: Message) {
        println!("Message received ({}#{} in {:?}): {}", msg.author.name,
                msg.author.discriminator, msg.channel_id.name(), msg.content);

        if msg.content == "!ping" {
            if let Err(why) = msg.channel_id.say("Pong!") {
                println!("Error sending message: {:?}", why);
            }
        } else if msg.content == "!game" {
            ctx.set_game_name("Wowzers");
        }
    }

    fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}


// -- Main --
fn main() {
    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN")
        .expect("Expected a token in the environment");
    let mut client = Client::new(&token, Handler).expect("Err creating client");

    if let Err(why) = client.start() {
        println!("Client error: {:?}", why);
    }
}
