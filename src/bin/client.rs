use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 🎵 Open Transcribe Client - Audio Recording & Transcription
#[derive(Parser)]
#[command(
    name = "open-transcribe-client",
    about = "🎵 Open Transcribe Client - Audio Recording & Transcription",
    long_about = "A client for the Open Transcribe server that can transcribe audio files or record and transcribe audio in real-time.",
    after_help = "EXAMPLES:\n    # Transcribe an existing audio file\n    open-transcribe-client file my_audio.wav\n\n    # Record 10 seconds of audio and transcribe\n    open-transcribe-client record --duration 10\n\n    # Record with custom audio settings\n    open-transcribe-client record --duration 15 --sample-rate 44100 --channels 2 --bit-depth 24\n\n    # Use a different server\n    open-transcribe-client file audio.wav --server-url http://my-server:8080"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Server URL
    #[arg(long, default_value = "http://localhost:8080", global = true)]
    server_url: String,

    /// Audio sample rate in Hz
    #[arg(long, default_value = "16000", global = true)]
    sample_rate: u32,

    /// Number of audio channels
    #[arg(long, default_value = "1", global = true)]
    channels: usize,

    /// Audio bit depth (16, 24, or 32)
    #[arg(long, default_value = "16", value_parser = validate_bit_depth, global = true)]
    bit_depth: u8,
}

#[derive(Subcommand)]
enum Commands {
    /// Transcribe an existing audio file
    #[command(name = "file")]
    TranscribeFile {
        /// Path to the audio file to transcribe
        audio_file: String,
    },
    /// Record audio and transcribe it
    #[command(name = "record")]
    Record {
        /// Recording duration in seconds
        #[arg(long, short = 'd', default_value = "5")]
        duration: u32,
    },
}

fn validate_bit_depth(s: &str) -> Result<u8, String> {
    match s.parse::<u8>() {
        Ok(16) | Ok(24) | Ok(32) => Ok(s.parse().unwrap()),
        Ok(_) => Err("Bit depth must be 16, 24, or 32".to_string()),
        Err(_) => Err("Invalid bit depth value".to_string()),
    }
}

#[derive(Debug)]
struct ClientConfig {
    server_url: String,
    audio_file: Option<String>,
    sample_rate: u32,
    channels: usize,
    bit_depth: u8,
    record_mode: bool,
    record_duration: u32,
}

impl From<Cli> for ClientConfig {
    fn from(cli: Cli) -> Self {
        match cli.command {
            Commands::TranscribeFile { audio_file } => ClientConfig {
                server_url: cli.server_url,
                audio_file: Some(audio_file),
                sample_rate: cli.sample_rate,
                channels: cli.channels,
                bit_depth: cli.bit_depth,
                record_mode: false,
                record_duration: 0,
            },
            Commands::Record { duration } => ClientConfig {
                server_url: cli.server_url,
                audio_file: None,
                sample_rate: cli.sample_rate,
                channels: cli.channels,
                bit_depth: cli.bit_depth,
                record_mode: true,
                record_duration: duration,
            },
        }
    }
}

fn record_audio(config: &ClientConfig) -> Result<Vec<u8>> {
    println!("🎤 Starting audio recording...");
    println!("   Duration: {} seconds", config.record_duration);
    println!("   Sample rate: {}Hz", config.sample_rate);
    println!("   Channels: {}", config.channels);
    println!("   Bit depth: {}", config.bit_depth);

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow!("No input device available"))?;

    println!("🎙️  Using input device: {}", device.name()?);

    // Get the default input config
    let mut supported_configs_range = device
        .supported_input_configs()
        .map_err(|e| anyhow!("Error querying input configs: {}", e))?;

    let supported_config = supported_configs_range
        .next()
        .ok_or_else(|| anyhow!("No supported config"))?
        .with_sample_rate(cpal::SampleRate(config.sample_rate));

    let config_cpal = supported_config.into();

    // Store recorded samples
    let recorded_samples = Arc::new(Mutex::new(Vec::new()));
    let recorded_samples_clone = Arc::clone(&recorded_samples);

    // Create the input stream
    let stream = device.build_input_stream(
        &config_cpal,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            let mut samples = recorded_samples_clone.lock().unwrap();
            samples.extend_from_slice(data);
        },
        |err| eprintln!("Error in audio stream: {}", err),
        None,
    )?;

    // Start recording
    stream.play()?;

    // Countdown before recording starts
    println!("🔴 Recording starting in...");
    for i in (1..=3).rev() {
        print!("   {}... ", i);
        std::io::stdout().flush().unwrap();
        std::thread::sleep(Duration::from_secs(1));
    }
    println!("🎙️  GO!");

    // Record for the specified duration
    for remaining in (1..=config.record_duration).rev() {
        if remaining % 5 == 0 || remaining <= 3 {
            println!("   {} seconds remaining...", remaining);
        }
        std::thread::sleep(Duration::from_secs(1));
    }

    // Stop recording
    drop(stream);
    println!("⏹️  Recording stopped");

    // Get the recorded samples
    let samples = recorded_samples.lock().unwrap();
    println!("📊 Recorded {} samples", samples.len());

    // Convert f32 samples to the specified bit depth
    let audio_bytes = match config.bit_depth {
        16 => {
            let mut bytes = Vec::new();
            for &sample in samples.iter() {
                let sample_i16 = (sample * i16::MAX as f32) as i16;
                bytes.extend_from_slice(&sample_i16.to_le_bytes());
            }
            bytes
        }
        24 => {
            let mut bytes = Vec::new();
            for &sample in samples.iter() {
                let sample_i32 = (sample * 8388607.0) as i32;
                let sample_bytes = sample_i32.to_le_bytes();
                bytes.extend_from_slice(&sample_bytes[0..3]);
            }
            bytes
        }
        32 => {
            let mut bytes = Vec::new();
            for &sample in samples.iter() {
                let sample_i32 = (sample * i32::MAX as f32) as i32;
                bytes.extend_from_slice(&sample_i32.to_le_bytes());
            }
            bytes
        }
        _ => return Err(anyhow!("Unsupported bit depth: {}", config.bit_depth)),
    };

    println!("💾 Converted to {} bytes", audio_bytes.len());
    Ok(audio_bytes)
}

async fn send_transcription_request(config: &ClientConfig) -> Result<Value> {
    let client = reqwest::Client::new();

    // Get audio data either from file or recording
    let audio_data = if config.record_mode {
        record_audio(config)?
    } else if let Some(ref file) = config.audio_file {
        if !Path::new(file).exists() {
            return Err(anyhow!("Audio file not found: {}", file));
        }
        fs::read(file).map_err(|e| anyhow!("Failed to read audio file: {}", e))?
    } else {
        return Err(anyhow!("No audio source specified"));
    };

    let source_info = if config.record_mode {
        "recorded audio".to_string()
    } else {
        format!("file: {}", config.audio_file.as_deref().unwrap())
    };

    println!(
        "📁 Audio source: {} ({} bytes)",
        source_info,
        audio_data.len()
    );

    // Create multipart form
    let filename = if config.record_mode {
        "recording.wav".to_string()
    } else {
        config.audio_file.clone().unwrap()
    };

    let form = reqwest::multipart::Form::new()
        .part(
            "audio",
            reqwest::multipart::Part::bytes(audio_data).file_name(filename),
        )
        .text("sample_rate", config.sample_rate.to_string())
        .text("channels", config.channels.to_string())
        .text("bit_depth", config.bit_depth.to_string());

    println!(
        "🚀 Sending transcription request to: {}/api/v1/transcribe",
        config.server_url
    );
    println!(
        "   Sample rate: {}Hz, Channels: {}, Bit depth: {}",
        config.sample_rate, config.channels, config.bit_depth
    );

    // Send the request
    let response = client
        .post(&format!("{}/api/v1/transcribe", config.server_url))
        .multipart(form)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to send request: {}", e))?;

    let status = response.status();
    let response_text = response
        .text()
        .await
        .map_err(|e| anyhow!("Failed to read response: {}", e))?;

    if !status.is_success() {
        return Err(anyhow!(
            "Server returned error {}: {}",
            status,
            response_text
        ));
    }

    let json: Value = serde_json::from_str(&response_text)
        .map_err(|e| anyhow!("Failed to parse JSON response: {}", e))?;

    Ok(json)
}

async fn check_server_health(server_url: &str) -> Result<()> {
    let client = reqwest::Client::new();

    println!("🔍 Checking server health at: {}/api/v1/health", server_url);

    let response = client
        .get(&format!("{}/api/v1/health", server_url))
        .send()
        .await
        .map_err(|e| anyhow!("Failed to connect to server: {}", e))?;

    if response.status().is_success() {
        println!("✅ Server is healthy");
        Ok(())
    } else {
        Err(anyhow!("Server health check failed: {}", response.status()))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();
    let config = ClientConfig::from(cli);

    println!("🎵 Open Transcribe Client");
    println!("========================");

    if config.record_mode {
        println!("🎤 Recording Mode");
        println!("   Duration: {} seconds", config.record_duration);
        println!(
            "   Audio format: {}Hz, {} channels, {}-bit",
            config.sample_rate, config.channels, config.bit_depth
        );
        println!("   Make sure your microphone is connected and working!");
        println!();
    } else {
        println!("📁 File Mode: {}", config.audio_file.as_ref().unwrap());
        println!();
    }

    // Check server health first
    if let Err(e) = check_server_health(&config.server_url).await {
        eprintln!("❌ {}", e);
        eprintln!("💡 Make sure the server is running: cargo run --bin server");
        return Err(e);
    }

    // Send transcription request
    match send_transcription_request(&config).await {
        Ok(result) => {
            println!("\n✅ Transcription completed!");
            println!("📝 Result:");
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Err(e) => {
            eprintln!("❌ Transcription failed: {}", e);
            return Err(e);
        }
    }

    Ok(())
}
