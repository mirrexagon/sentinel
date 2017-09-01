// --- Use --- //
use std::collections::HashMap;

use discord;
use discord::{Discord, Connection, State};
use discord::model::Event as DiscordEvent;

use {Error, Result};

use modules::dj::Dj;
// --- ==== --- //


pub trait Module {
    /// Returns the internal name of the module. This must be unique.
    fn get_name(&self) -> &str;

    fn process_event(&mut self, bot: &mut Bot, event: &Event) -> Result<()>;
}


/// Events that modules can receive.
#[derive(Clone, Debug)]
pub enum Event {
    Discord(DiscordEvent),

    Load,
    Unload,
}


pub struct Bot {
    pub discord: Discord,
    pub connection: Option<Connection>,
    pub state: Option<State>,

    modules: HashMap<String, Box<Module>>,
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

            modules: HashMap::new(),
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
            match self.recv_event() {
                Ok(event) => self.process_event(&Event::Discord(event)),
                Err(err) => {
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
                }
            }
        }

        Ok(())
    }

    /// Have all modules process an event.
    pub fn process_event(&mut self, event: &Event) -> Result<()> {
        for (name, module) in &mut self.modules {
            // TODO: Collect errors from each module.
            module.process_event(&mut self, &event);
        }

        Ok(())
    }

    /// Receive an event from Discord and update internal state.
    fn recv_event(&mut self) -> Result<DiscordEvent> {
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

        Ok(event)
    }

    /// Attempts to load a module.
    pub fn load_module<T: Module>(&mut self, module: T) -> Result<()> {
        if self.modules.contains_key(module.get_name()) {
            return Err(Error::ModuleAlreadyLoaded);
        }

        let module = Box::new(module);

        // TODO: Don't ignore errors on load?
        module.process_event(self, &Event::Load);

        Ok(())
    }

    pub fn unload_module(&mut self, name: &str) -> Result<()> {
        if !self.modules.contains_key(name) {
            return Err(Error::ModuleNotLoaded)
        }

        let module = self.modules.remove(name).unwrap();

        // TODO: Don't ignore errors on unload?
        module.process_event(self, &Event::Unload);

        Ok(())
    }
}
