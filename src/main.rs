// Future functionality:
// - Voice control
// - Auth, admin, help
// - Markov: per user, per channel, everything


// --- External crates --- //
#[macro_use]
extern crate log;
extern crate env_logger;

extern crate discord;

extern crate regex;

#[macro_use]
extern crate lazy_static;
// --- ==== --- //


// --- Use --- //
use std::env;
use std::process;

use discord::{Discord, Connection, State};
use discord::model::Event as DiscordEvent;
use discord::Result as DiscordResult;
use discord::voice::AudioSource;

use regex::Regex;

use std::io::{Read, Write};
// --- ==== --- //


// --- Constants --- //
// --- ==== --- //


// --- espeak source --- //
struct VecStream(Vec<u8>, usize);

impl VecStream {
    pub fn new(v: Vec<u8>) -> Self {
        VecStream(v, 0)
    }
}

impl Read for VecStream {
    fn read(&mut self, buf: &mut [u8]) -> ::std::io::Result<usize> {
        for i in 0..buf.len() {
            if self.1 >= self.0.len() {
                return Ok(i);
            }

            buf[i] = self.0[self.1];
            self.1 += 1;
        }

        Ok(buf.len())
    }
}

// ---

fn create_espeak_source(text: &str) -> DiscordResult<Box<discord::voice::AudioSource>> {
    let TMP_FILE = "/tmp/espeak.wav";
    let TMP_FILE_2 = "/tmp/espeak-conv.wav";
    let stereo = false;

    use std::process::{Command, Stdio};
    use std::fs;

    // ---

	let mut child = Command::new("espeak")
		.arg("--stdout")
		//.arg("-p").arg("0")
		//.arg("-s").arg("200")
		//.arg("-v").arg("english-us")
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::null())
		.spawn()?;

    {

        let mut stdin = child.stdin.as_mut().expect("Failed to open stdin");
        stdin.write_all(text.as_bytes())?;
    }

    let status = child.wait();

    // ---
	
	let output = Command::new("ffmpeg")
		.arg("-i").arg(TMP_FILE)
		.args(&[
			"-f", "s16le",
			"-ac", if stereo { "2" } else { "1" },
			"-ar", "48000",
			"-acodec", "pcm_s16le",
			TMP_FILE_2])
		.output()?;

	// ---

    let data: Vec<u8>;

    {
        let f = fs::File::open(TMP_FILE_2)?;
        data = f.bytes().map(|b| b.unwrap()).collect();
    }

    // ---

    fs::remove_file(TMP_FILE);
    fs::remove_file(TMP_FILE_2);

    // ---

	Ok(discord::voice::create_pcm_source(stereo, VecStream::new(data)))
}
// --- ==== --- //


// --- ==== --- //
// --- ==== --- //


// --- Bot type --- //
struct Bot {
    discord: Discord,
    connection: Option<Connection>,
    state: Option<State>,

    current_server: Option<discord::model::ServerId>,
    in_voice_channel: bool,
}

impl Bot {
    pub fn from_bot_token(bot_token: &str) -> DiscordResult<Self> {
        info!("Logging in...");
        let discord = Discord::from_bot_token(bot_token)?;
        info!("Logged in!");


        Ok(Bot {
            discord: discord,
            connection: None,
            state: None,

            current_server: None,
            in_voice_channel: false,
        })
    }

    pub fn connect(&mut self) -> DiscordResult<()> {
        info!("Connecting...");

        let (connection, ready) = self.discord.connect()?;
        let state = State::new(ready);
        connection.sync_calls(&state.all_private_channels());

        self.connection = Some(connection);
        self.state = Some(state);

        info!("Connected!.");

        Ok(())
    }

    fn process_event(&mut self, event: &DiscordEvent) -> DiscordResult<()> {
        lazy_static! {
            static ref JOIN_COMMAND_REGEX: Regex = Regex::new(r"^&join").unwrap();
            static ref LEAVE_COMMAND_REGEX: Regex = Regex::new(r"^&leave").unwrap();
            static ref QUIT_COMMAND_REGEX: Regex = Regex::new(r"^&quit").unwrap();
            static ref SAY_COMMAND_REGEX: Regex = Regex::new(r"^&say (.+)$").unwrap();
        }

        match *event {
            DiscordEvent::MessageCreate(ref message) => {
                if JOIN_COMMAND_REGEX.is_match(&message.content) {
                    {
                        let reply = format!("I heard you.");
                        self.discord.send_message(message.channel_id, &reply, "", false)?;
                    }

                    for s in self.state.as_ref().unwrap().servers() {
                        println!("{}", s.name);
                    }

                    let servers = self.state.as_ref().unwrap().unavailable_servers();
                    println!("{:?}", servers);

                    match self.state.as_ref().unwrap().find_voice_user(message.author.id) {
                        Some((server_id_maybe, channel_id)) => {
                            let voice = self.connection.as_mut().unwrap().voice(server_id_maybe);
                            voice.connect(channel_id);
                            voice.set_deaf(true);

                            self.in_voice_channel = true;
                            self.current_server = server_id_maybe;
                        }

                        _ => {
                            let reply = format!("You're not in a voice channel on a server.");
                            self.discord.send_message(message.channel_id, &reply, "", false)?;
                        }
                    };

                } else if LEAVE_COMMAND_REGEX.is_match(&message.content) {
                    let voice = self.connection.as_mut().unwrap().voice(self.current_server);
                    voice.stop();
                    voice.disconnect();
                    self.in_voice_channel = false;

                } else if QUIT_COMMAND_REGEX.is_match(&message.content) {
                    let reply = format!("Quitting!");
                    self.discord.send_message(message.channel_id, &reply, "", false)?;

                    {
                        let voice = self.connection.as_mut().unwrap().voice(self.current_server);
                        voice.disconnect();
                        self.in_voice_channel = false;
                    }

                    let mut conn = self.connection.take().unwrap();
                    conn.shutdown()?;

                    process::exit(0);
                } else if SAY_COMMAND_REGEX.is_match(&message.content) {
                    if self.in_voice_channel {
                        let captures = SAY_COMMAND_REGEX.captures(&message.content).unwrap();
                        let text = &captures[1];

                        {
                            let reply = format!("Saying: {}", text);
                            self.discord.send_message(message.channel_id, &reply, "", false)?;
                        }

                        let source = create_espeak_source(text)?;

                        let voice = self.connection.as_mut().unwrap().voice(self.current_server);
                        voice.stop();
                        voice.play(source);
                    } else {
                        let reply = format!("I'm not in a voice channel.");
                        self.discord.send_message(message.channel_id, &reply, "", false)?;
                    }

                }
            }

            _ => {}
        };

        Ok(())
    }

    pub fn run(&mut self) -> DiscordResult<()> {
        loop {
            match self.recv_event() {
                Ok(event) => self.process_event(&event)?,
                Err(err) => {
                    warn!("Receive error: {:?}", err);

                    if let discord::Error::WebSocket(..) = err {
                        // Handle the websocket connection being dropped.
                        self.connect()?;

                        info!("Reconnected successfully.");
                    }

                    if let discord::Error::Closed(..) = err {
                        warn!("Connection closed.");
                        return Err(err);
                    }
                }
            }
        }
    }

    /// Receive an event from Discord and update internal state.
    fn recv_event(&mut self) -> DiscordResult<DiscordEvent> {
        let connection = self.connection.as_mut();
        let state = self.state.as_mut();

        if let None = connection {
            panic!("Tried to receive an event while not connected!")
        }

        let connection = connection.unwrap();
        let state = state.unwrap();

        // ---

        let event = connection.recv_event()?;
        state.update(&event);

        Ok(event)
    }
}
// --- ==== --- //


pub fn main() {
    env_logger::init();

    let mut bot = Bot::from_bot_token(&env::var("DISCORD_TOKEN").expect("Expected token")).unwrap();

    bot.connect().expect("Connecting failed");
    bot.run().unwrap();
}
