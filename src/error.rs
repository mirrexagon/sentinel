// --- Use --- //
use discord;
// --- ==== --- //

pub type Result<T> = ::std::result::Result<T, Error>;

quick_error! {
    #[derive(Debug)]
    pub enum Error {
        Discord(err: discord::Error) {
            description(err.description())
            cause(err)
            display("Discord error: {}", err)
            from()
        }

        NotConnected {}

        ModuleNotLoaded {}
        ModuleAlreadyLoaded {}
    }
}
