// --- Use --- //
use discord;
use discord::{Discord, Connection, State};
use discord::model::Event as DiscordEvent;

use {Error, Result};

use bot::{Bot, Module, Event};
// --- ==== --- //


#[derive(Debug)]
pub struct Dj;

impl Module for Dj {
    fn get_name(&self) -> &'static str {
        "dj"
    }

    fn process_event(&mut self, bot: &mut Bot, event: &Event) -> Result<()> {
        let connection = bot.connection.as_mut();
        let state = bot.state.as_mut();

        if let None = connection {
            return Err(Error::NotConnected);
        }

        let connection = connection.unwrap();
        let state = state.unwrap();

        // ---

        // Copied from the dj example of the discord crate.
        if let Event::Discord(ref event) = *event {
            match *event {
                DiscordEvent::MessageCreate(ref message) => {
                    use std::ascii::AsciiExt;
                    // safeguard: stop if the message is from us
                    if message.author.id == state.user().id {
                        return Ok(());
                    }

                    // reply to a command if there was one
                    let mut split = message.content.split(' ');
                    let first_word = split.next().unwrap_or("");
                    let argument = split.next().unwrap_or("");

                    if first_word.eq_ignore_ascii_case("!dj") {
                        let vchan = state.find_voice_user(message.author.id);
                        if argument.eq_ignore_ascii_case("stop") {
                            vchan.map(|(sid, _)| connection.voice(sid).stop());
                            info!("[DJ] Stopping.");
                        } else if argument.eq_ignore_ascii_case("quit") {
                            info!("[DJ] Quitting.");
                            vchan.map(|(sid, _)| connection.drop_voice(sid));
                        } else {
                            info!("[DJ] Playing: {}", argument);
                            let output = if let Some((server_id, channel_id)) = vchan {
                                match discord::voice::open_ytdl_stream(argument) {
                                    Ok(stream) => {
                                        let voice = connection.voice(server_id);
                                        voice.set_deaf(true);
                                        voice.connect(channel_id);
                                        voice.play(stream);
                                        String::new()
                                    }
                                    Err(error) => format!("Error: {}", error),
                                }
                            } else {
                                "You must be in a voice channel to DJ.".to_owned()
                            };

                            if !output.is_empty() {
                                if let Err(err) = bot.discord
                                    .send_message(message.channel_id, &output, "", false) {
                                    warn!("[DJ] Error sending response: {:?}", err);
                                }
                            }
                        }
                    }

                    Ok(())
                }

                DiscordEvent::VoiceStateUpdate(server_id, _) => {
                    // If someone moves/hangs up, and we are in a voice channel,
                    if let Some(cur_channel) = connection.voice(server_id).current_channel() {
                        // and our current voice channel is empty, disconnect from voice
                        match server_id {
                            Some(server_id) => {
                                if let Some(srv) = state.servers()
                                    .iter()
                                    .find(|srv| srv.id == server_id) {
                                    if srv.voice_states
                                        .iter()
                                        .filter(|vs| vs.channel_id == Some(cur_channel))
                                        .count() <= 1 {
                                        connection.voice(Some(server_id)).disconnect();
                                    }
                                }
                            }
                            None => {
                                if let Some(call) = state.calls().get(&cur_channel) {
                                    if call.voice_states.len() <= 1 {
                                        connection.voice(server_id).disconnect();
                                    }
                                }
                            }
                        }
                    }

                    Ok(())
                }

                _ => Ok(()), // discard other events
            }
        } else {
            Ok(())
        }
    }
}
