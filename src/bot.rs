// --- Use --- //
use discord;
use discord::{Discord, Connection, State};
use discord::model::Event;

use {Error, Result};
// --- ==== --- //


// --- Bot --- //
pub struct Bot {
    discord: Discord,
    connection: Option<Connection>,
    state: Option<State>,
}


impl Bot {
    pub fn from_bot_token(bot_token: &str) -> Result<Self> {
        info!("Logging in...");
        let discord = Discord::from_bot_token(bot_token)?;
        info!("Logged in!");

        Ok(Bot {
            discord: discord,
            connection: None,
            state: None,
        })
    }

    pub fn connect(&mut self) -> Result<()> {
        info!("Connecting...");

        let (connection, ready) = self.discord.connect()?;
        let state = State::new(ready);
        connection.sync_calls(&state.all_private_channels());

        self.connection = Some(connection);
        self.state = Some(state);

        info!("Connected!.");

        Ok(())
    }

    pub fn run(&mut self) -> Result<()> {
        loop {
            if let Err(err) = self.process() {
                warn!("Receive error: {:?}", err);

                if let Error::Discord(discord::Error::WebSocket(..)) = err {
                    // Handle the websocket connection being dropped.
                    self.connect()?;

                    info!("Reconnected successfully.");
                }

                if let Error::Discord(discord::Error::Closed(..)) = err {
                    warn!("Connection closed.");
                    return Err(err);
                }

                continue;
            }
        }
    }

    pub fn process(&mut self) -> Result<()> {
        let event = {
            let connection = self.connection.as_mut();
            let state = self.state.as_mut();

            if let None = connection {
                return Err(Error::NotConnected);
            }

            let connection = connection.unwrap();
            let state = state.unwrap();

            // ---

            let event = connection.recv_event()?;
            state.update(&event);

            event
        };

        // ---

        self.process_dj(&event)?;

        // ---

        Ok(())
    }

    // ---

    fn process_dj(&mut self, event: &Event) -> Result<()> {

        let connection = self.connection.as_mut();
        let state = self.state.as_mut();

        if let None = connection {
            return Err(Error::NotConnected);
        }

        let connection = connection.unwrap();
        let state = state.unwrap();

        // ---

        // Copied from the dj example of the discord crate.
        match *event {
            Event::MessageCreate(ref message) => {
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
                            if let Err(err) = self.discord
                                .send_message(message.channel_id, &output, "", false) {
                                warn!("[DJ] Error sending response: {:?}", err);
                            }
                        }
                    }
                }

                Ok(())
            }

            Event::VoiceStateUpdate(server_id, _) => {
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
    }
}
// --- ==== --- //
