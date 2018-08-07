// -- Use --
use serenity::prelude::*;

use talklike;

// -- Funcs --
fn save_data(ctx: &Context) {
    talklike::save(ctx);
}

// -- Commands --
pub mod commands {
    use std::process;

    use super::save_data;

    command!(
        save(ctx, msg, _args) {
            msg.channel_id.say("Saving data...");
            save_data(&ctx);
            msg.channel_id.say("Saved.");
        }
    );

    command!(
        quit(ctx, msg, _args) {
            save_data(&ctx);
            msg.channel_id.say("Bye!");
            ctx.quit();
            process::exit(0);
        }
    );
}
