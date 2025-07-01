use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "open-transcribe",
    about = "Open Transcribe - Audio Recording & Transcription",
    long_about = "A unified tool for transcribing audio files or recording and transcribing audio in real-time, with built-in server functionality.",
    after_help = "EXAMPLES:\n    # Start the transcription server\n    open-transcribe serve\n\n    # Transcribe an existing audio file\n    open-transcribe file my_audio.wav\n\n    # Record 10 seconds of audio and transcribe\n    open-transcribe record --duration 10\n\n    # Record with custom audio settings\n    open-transcribe record --duration 15 --sample-rate 44100 --channels 2 --bit-depth 24\n\n    # Use a different server when in client mode\n    open-transcribe file audio.wav --server-url http://my-server:8080"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(name = "serve")]
    Serve {
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        #[arg(long, default_value = "8080")]
        port: u16,
    },
    #[command(name = "file")]
    TranscribeFile {
        audio_file: String,

        #[arg(long, default_value = "http://localhost:8080")]
        server_url: String,

        #[arg(long, default_value = "16000")]
        sample_rate: u32,

        #[arg(long, default_value = "1")]
        channels: usize,

        #[arg(long, default_value = "16", value_parser = validate_bit_depth)]
        bit_depth: u8,
    },
    #[command(name = "record")]
    Record {
        #[arg(long, short = 'd', default_value = "5")]
        duration: u32,

        #[arg(long, default_value = "http://localhost:8080")]
        server_url: String,

        #[arg(long, default_value = "16000")]
        sample_rate: u32,

        #[arg(long, default_value = "1")]
        channels: usize,

        #[arg(long, default_value = "16", value_parser = validate_bit_depth)]
        bit_depth: u8,
    },
}

pub fn validate_bit_depth(s: &str) -> Result<u8, String> {
    match s.parse::<u8>() {
        Ok(16) | Ok(24) | Ok(32) => Ok(s.parse().unwrap()),
        Ok(_) => Err("Bit depth must be 16, 24, or 32".to_string()),
        Err(_) => Err("Invalid bit depth value".to_string()),
    }
}
