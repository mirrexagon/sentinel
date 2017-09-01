// --- External crates --- //
#[macro_use]
extern crate log;
extern crate env_logger;

#[macro_use]
extern crate quick_error;

extern crate discord;
// --- ==== --- //


// --- Modules --- //
mod error;
mod bot;
mod modules;
// --- ==== --- //


// --- Re-exports --- //
pub use error::Error;
pub use error::Result;
// --- ==== --- //


// --- Use --- //
use std::env;

use bot::Bot;
// --- ==== --- //


// Future functionality:
// - Auth, admin, help
// - DJ
// - Acapella box
// - Markov: per user, per channel, everything
// - Allow per-user colors by giving personal roles.

pub fn main() {
    env_logger::init().expect("Failed to intialize env_logger");

    let mut bot = Bot::from_bot_token(&env::var("DISCORD_TOKEN").expect("Expected token")).unwrap();

    // ---

    bot.load_module(modules::dj::Dj);

    // ---

    bot.connect().expect("Connecting failed");
    bot.run().unwrap();
}
