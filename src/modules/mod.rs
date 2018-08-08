// -- Use --
use std::path;

use serenity::framework::standard::StandardFramework;
use serenity::model::channel::Message;

use serenity::prelude::*;

// -- Type aliases --
type SerenityResult<T> = Result<T, SerenityError>;

// -- Trait --
pub trait Module {
    fn init_framework(framework: StandardFramework) -> StandardFramework;
    fn init_client(client: &mut client) -> SerenityResult<()>;

    fn on_message(ctx: &Context, msg: &Message);
    
    fn save_data(ctx: &Context, data_dir: &path::Path);
}

// -- Modules --
pub mod meta;
pub mod talklike;
