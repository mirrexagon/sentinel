//! Text-to-speech.


// --- Use --- //
use std::io::Read;
use std::io::Result as IoResult;
use std::process::{Command, Stdio};
use std::fs;

use discord::Result as DiscordResult;
use discord::voice::AudioSource;
// --- ==== --- //


// --- Vec stream --- //
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
// --- ==== --- //


// --- Main trait --- //
pub trait TtsProvider {
    fn generate(text: &str) -> IoResult<Box<AudioSource>>;
}
// --- ==== --- //


// --- espeak TTS --- //
const ESPEAK_OUT_PATH: &str = "/tmp/espeak-raw.wav";
const ESPEAK_CONVERT_PATH: &str = "/tmp/espeak-resampled.wav";

// ---

pub struct EspeakTts {

}

impl EspeakTts {

}

impl TtsProvider for EspeakTts {
    fn generate(text: &str) -> IoResult<Box<AudioSource>> {
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
}
// --- ==== --- //
