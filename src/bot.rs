// --- Use --- //
use discord;
use discord::{Discord, State, Connection};
use discord::model::Event;

use {Error, Result};
// --- ==== --- //


// --- Bot --- //
pub struct Bot {
    discord: Discord,
    connstate: Option<ConnectionState>,
}

struct ConnectionState {
    connection: Connection,
    state: State,
}


impl Bot {
    pub fn from_bot_token(bot_token: &str) -> Result<Self> {
    	let discord = Discord::from_bot_token(bot_token)?;

    	Ok(Bot {
    	   discord: discord,
    	   connstate: None,
    	})
    }

    pub fn connect(&mut self) -> Result<()> {
        let (mut connection, ready) = self.discord.connect()?;
        let mut state = State::new(ready);
    	connection.sync_calls(&state.all_private_channels());

        self.connstate = Some(ConnectionState { connection, state });

        Ok(())
    }

    fn get_connstate(&mut self) -> Result<&mut ConnectionState> {
        if let Some(ref mut connstate) = self.connstate {
            Ok(connstate)
        } else {
            Err(Error::NotConnected)
        }
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

                continue
            }
        }
    }

    pub fn process(&mut self) -> Result<()> {
        let event = {
            let connstate = self.get_connstate()?;
            let event = connstate.connection.recv_event()?;

            connstate.state.update(&event);

            event
        };

        // ---

        self.process_dj(&event)?;

        // ---

        Ok(())
    }

    // ---

    fn process_dj(&mut self, event: &Event) -> Result<()> {
        // Copied from the dj example of the discord crate.

        let connstate = self.get_connstate()?;

		match *event {
			Event::MessageCreate(ref message) => {
				use std::ascii::AsciiExt;
				// safeguard: stop if the message is from us
				if message.author.id == connstate.state.user().id {
					return Ok(());
				}

				// reply to a command if there was one
				let mut split = message.content.split(' ');
				let first_word = split.next().unwrap_or("");
				let argument = split.next().unwrap_or("");

				if first_word.eq_ignore_ascii_case("!dj") {
					let vchan = connstate.state.find_voice_user(message.author.id);
					if argument.eq_ignore_ascii_case("stop") {
						vchan.map(|(sid, _)| connstate.connection.voice(sid).stop());
					   info!("[DJ] Stopping.");
					} else if argument.eq_ignore_ascii_case("quit") {
					   info!("[DJ] Quitting.");
						vchan.map(|(sid, _)| connstate.connection.drop_voice(sid));
					} else {
					   println!("[DJ] Playing: {}", argument);
						let output = if let Some((server_id, channel_id)) = vchan {
							match discord::voice::open_ytdl_stream(argument) {
								Ok(stream) => {
									let voice = connstate.connection.voice(server_id);
									voice.set_deaf(true);
									voice.connect(channel_id);
									voice.play(stream);
									String::new()
								},
								Err(error) => format!("Error: {}", error),
							}
						} else {
							"You must be in a voice channel to DJ.".to_owned()
						};

						if !output.is_empty() {
							self.discord.send_message(message.channel_id, &output, "", false);
						}
					}
				}

                Ok(())
			}

			Event::VoiceStateUpdate(server_id, _) => {
				// If someone moves/hangs up, and we are in a voice channel,
				if let Some(cur_channel) = connstate.connection.voice(server_id).current_channel() {
					// and our current voice channel is empty, disconnect from voice
					match server_id {
						Some(server_id) => if let Some(srv) = connstate.state.servers().iter().find(|srv| srv.id == server_id) {
							if srv.voice_states.iter().filter(|vs| vs.channel_id == Some(cur_channel)).count() <= 1 {
								connstate.connection.voice(Some(server_id)).disconnect();
							}
						},
						None => if let Some(call) = connstate.state.calls().get(&cur_channel) {
							if call.voice_states.len() <= 1 {
								connstate.connection.voice(server_id).disconnect();
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
