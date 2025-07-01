use open_transcribe::dto;
use open_transcribe::whisper;

use actix_cors::Cors;
use actix_multipart::{Field, Multipart};
use actix_web::{App, HttpResponse, HttpServer, Responder, get, middleware::Logger, post, web};
use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use futures_util::TryStreamExt;
use log::{debug, error, info, warn};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use whisper::config::WhisperConfig;
use whisper::transcriber::{InputAudio, SimpleTranscriber};

/// 🎵 Open Transcribe - Audio Recording & Transcription
#[derive(Parser)]
#[command(
    name = "open-transcribe",
    about = "🎵 Open Transcribe - Audio Recording & Transcription",
    long_about = "A unified tool for transcribing audio files or recording and transcribing audio in real-time, with built-in server functionality.",
    after_help = "EXAMPLES:\n    # Start the transcription server\n    open-transcribe serve\n\n    # Transcribe an existing audio file\n    open-transcribe file my_audio.wav\n\n    # Record 10 seconds of audio and transcribe\n    open-transcribe record --duration 10\n\n    # Record with custom audio settings\n    open-transcribe record --duration 15 --sample-rate 44100 --channels 2 --bit-depth 24\n\n    # Use a different server when in client mode\n    open-transcribe file audio.wav --server-url http://my-server:8080"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the transcription server
    #[command(name = "serve")]
    Serve {
        /// Server bind address
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Server port
        #[arg(long, default_value = "8080")]
        port: u16,
    },
    /// Transcribe an existing audio file
    #[command(name = "file")]
    TranscribeFile {
        /// Path to the audio file to transcribe
        audio_file: String,

        /// Server URL
        #[arg(long, default_value = "http://localhost:8080")]
        server_url: String,

        /// Audio sample rate in Hz
        #[arg(long, default_value = "16000")]
        sample_rate: u32,

        /// Number of audio channels
        #[arg(long, default_value = "1")]
        channels: usize,

        /// Audio bit depth (16, 24, or 32)
        #[arg(long, default_value = "16", value_parser = validate_bit_depth)]
        bit_depth: u8,
    },
    /// Record audio and transcribe it
    #[command(name = "record")]
    Record {
        /// Recording duration in seconds
        #[arg(long, short = 'd', default_value = "5")]
        duration: u32,

        /// Server URL
        #[arg(long, default_value = "http://localhost:8080")]
        server_url: String,

        /// Audio sample rate in Hz
        #[arg(long, default_value = "16000")]
        sample_rate: u32,

        /// Number of audio channels
        #[arg(long, default_value = "1")]
        channels: usize,

        /// Audio bit depth (16, 24, or 32)
        #[arg(long, default_value = "16", value_parser = validate_bit_depth)]
        bit_depth: u8,
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

// Server functionality
struct AppState {
    transcriber: SimpleTranscriber,
}

#[get("/api/v1/health")]
async fn health_check() -> impl Responder {
    debug!("Health check endpoint called");
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "message": "Whisper transcription service is running"
    }))
}

#[post("/api/v1/transcribe")]
async fn transcribe_upload(data: web::Data<AppState>, mut payload: Multipart) -> impl Responder {
    debug!("Transcription request received");

    let mut audio_data: Option<Vec<u8>> = None;
    let mut sample_rate: u32 = 16000; // 16kHz
    let mut channels: usize = 1;
    let mut bit_depth: u8 = 16;

    // Process multipart fields
    while let Some(field) = payload.try_next().await.unwrap_or(None) {
        match field.name() {
            Some("audio") => match read_field_data(field).await {
                Ok(data) => {
                    debug!("Audio data received: {} bytes", data.len());
                    audio_data = Some(data);
                }
                Err(e) => {
                    error!("Failed to read audio data: {}", e);
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": "Failed to read audio data"
                    }));
                }
            },
            Some("sample_rate") => {
                if let Ok(field_data) = read_field_data(field).await {
                    if let Ok(text) = String::from_utf8(field_data) {
                        sample_rate = text.trim().parse().unwrap_or(16000);
                        debug!("Sample rate set to: {}", sample_rate);
                    }
                }
            }
            Some("channels") => {
                if let Ok(field_data) = read_field_data(field).await {
                    if let Ok(text) = String::from_utf8(field_data) {
                        channels = text.trim().parse().unwrap_or(1);
                        debug!("Channels set to: {}", channels);
                    }
                }
            }
            Some("bit_depth") => {
                if let Ok(field_data) = read_field_data(field).await {
                    if let Ok(text) = String::from_utf8(field_data) {
                        bit_depth = text.trim().parse().unwrap_or(16);
                        debug!("Bit depth set to: {}", bit_depth);
                    }
                }
            }
            _ => continue,
        }
    }

    let audio_bytes = match audio_data {
        Some(data) => data,
        None => {
            warn!("No audio file provided in transcription request");
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "No audio file provided"
            }));
        }
    };

    info!(
        "Processing audio: {} bytes, {}Hz, {} channels, {} bit",
        audio_bytes.len(),
        sample_rate,
        channels,
        bit_depth
    );

    // Convert raw audio bytes to f32 samples
    let audio_samples = match convert_audio_bytes_to_samples(&audio_bytes, bit_depth) {
        Ok(samples) => {
            debug!(
                "Successfully converted {} bytes to {} samples",
                audio_bytes.len(),
                samples.len()
            );
            samples
        }
        Err(error_msg) => {
            error!("Failed to convert audio bytes to samples: {}", error_msg);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": error_msg
            }));
        }
    };

    transcribe_audio_samples(&data.transcriber, audio_samples, sample_rate, channels).await
}

async fn read_field_data(mut field: Field) -> Result<Vec<u8>, actix_web::Error> {
    let mut data = Vec::new();
    while let Some(chunk) = field.try_next().await? {
        data.extend_from_slice(&chunk);
    }
    debug!("Read field data: {} bytes", data.len());
    Ok(data)
}

fn convert_audio_bytes_to_samples(audio_bytes: &[u8], bit_depth: u8) -> Result<Vec<f32>, String> {
    debug!(
        "Converting {} bytes of {}-bit audio to samples",
        audio_bytes.len(),
        bit_depth
    );

    match bit_depth {
        16 => {
            if audio_bytes.len() % 2 != 0 {
                error!(
                    "Invalid 16-bit audio data: odd number of bytes ({})",
                    audio_bytes.len()
                );
                return Err("Invalid 16-bit audio data: odd number of bytes".to_string());
            }
            let samples = audio_bytes
                .chunks_exact(2)
                .map(|chunk| {
                    let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                    sample as f32 / i16::MAX as f32
                })
                .collect();
            debug!(
                "Converted 16-bit audio to {} samples",
                audio_bytes.len() / 2
            );
            Ok(samples)
        }
        24 => {
            if audio_bytes.len() % 3 != 0 {
                error!(
                    "Invalid 24-bit audio data: byte count ({}) not divisible by 3",
                    audio_bytes.len()
                );
                return Err("Invalid 24-bit audio data: byte count not divisible by 3".to_string());
            }
            let samples = audio_bytes
                .chunks_exact(3)
                .map(|chunk| {
                    let sample = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], 0]) >> 8;
                    sample as f32 / 8388607.0
                })
                .collect();
            debug!(
                "Converted 24-bit audio to {} samples",
                audio_bytes.len() / 3
            );
            Ok(samples)
        }
        32 => {
            if audio_bytes.len() % 4 != 0 {
                error!(
                    "Invalid 32-bit audio data: byte count ({}) not divisible by 4",
                    audio_bytes.len()
                );
                return Err("Invalid 32-bit audio data: byte count not divisible by 4".to_string());
            }
            let samples = audio_bytes
                .chunks_exact(4)
                .map(|chunk| {
                    let sample = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    sample as f32 / i32::MAX as f32
                })
                .collect();
            debug!(
                "Converted 32-bit audio to {} samples",
                audio_bytes.len() / 4
            );
            Ok(samples)
        }
        _ => {
            error!("Unsupported bit depth: {}", bit_depth);
            Err(format!("Unsupported bit depth: {}", bit_depth))
        }
    }
}

async fn transcribe_audio_samples(
    transcriber: &SimpleTranscriber,
    audio_samples: Vec<f32>,
    sample_rate: u32,
    channels: usize,
) -> HttpResponse {
    if audio_samples.is_empty() {
        warn!("No audio data provided for transcription");
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "No audio data provided"
        }));
    }

    info!(
        "Starting transcription: {} samples, {}Hz, {} channels",
        audio_samples.len(),
        sample_rate,
        channels
    );

    let input_audio = InputAudio {
        data: &audio_samples,
        sample_rate,
        channels,
    };

    match transcriber.transcribe(&input_audio) {
        Ok(output) => {
            info!(
                "Transcription completed successfully: {} segments, {} characters",
                output.segments.len(),
                output.combined.len()
            );

            let segments: Vec<dto::TranscriptionSegment> = output
                .segments
                .into_iter()
                .map(|seg| dto::TranscriptionSegment {
                    start: seg.start,
                    end: seg.end,
                    text: seg.text,
                    confidence: seg.confidence,
                })
                .collect();

            HttpResponse::Ok().json(dto::TranscriptionDto {
                text: output.combined,
                segments: Some(segments),
            })
        }
        Err(e) => {
            error!("Transcription failed: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Transcription failed: {}", e)
            }))
        }
    }
}

// Client functionality
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

async fn run_server(host: String, port: u16) -> std::io::Result<()> {
    info!("Starting Whisper transcription service");
    info!("Initializing Whisper transcriber...");

    let config = WhisperConfig::default();
    info!(
        "Using configuration: model_path={:?}, use_gpu={}, language={}, num_threads={}",
        config.model_path, config.use_gpu, config.language, config.num_threads
    );

    let transcriber = match SimpleTranscriber::new(config) {
        Ok(t) => {
            info!("Whisper transcriber initialized successfully");
            t
        }
        Err(e) => {
            error!("Failed to initialize transcriber: {}", e);
            std::process::exit(1);
        }
    };

    let app_state = web::Data::new(AppState { transcriber });

    info!("Starting HTTP server on {}:{}", host, port);

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .app_data(web::JsonConfig::default().limit(50 * 1024 * 1024)) // 50MB
            .app_data(
                actix_multipart::form::MultipartFormConfig::default()
                    .total_limit(100 * 1024 * 1024), // 100MB
            )
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allow_any_method()
                    .allow_any_header()
                    .max_age(3600),
            )
            .wrap(Logger::default())
            .service(health_check)
            .service(transcribe_upload)
    })
    .bind((host.as_str(), port))?
    .run()
    .await
}

async fn run_client(config: ClientConfig) -> Result<()> {
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
        eprintln!("💡 Make sure the server is running: open-transcribe serve");
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

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { host, port } => {
            println!("🎵 Open Transcribe Server");
            println!("========================");
            run_server(host, port)
                .await
                .map_err(|e| anyhow!("Server error: {}", e))?;
        }
        Commands::TranscribeFile {
            audio_file,
            server_url,
            sample_rate,
            channels,
            bit_depth,
        } => {
            let config = ClientConfig {
                server_url,
                audio_file: Some(audio_file),
                sample_rate,
                channels,
                bit_depth,
                record_mode: false,
                record_duration: 0,
            };
            run_client(config).await?;
        }
        Commands::Record {
            duration,
            server_url,
            sample_rate,
            channels,
            bit_depth,
        } => {
            let config = ClientConfig {
                server_url,
                audio_file: None,
                sample_rate,
                channels,
                bit_depth,
                record_mode: true,
                record_duration: duration,
            };
            run_client(config).await?;
        }
    }

    Ok(())
}
