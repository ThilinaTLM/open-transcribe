use std::env;
use std::fs;
use std::path::Path;
use anyhow::{anyhow, Result};
use serde_json::Value;

#[derive(Debug)]
struct ClientConfig {
    server_url: String,
    audio_file: String,
    sample_rate: u32,
    channels: usize,
    bit_depth: u8,
}

impl ClientConfig {
    fn from_args() -> Result<Self> {
        let args: Vec<String> = env::args().collect();
        
        if args.len() < 2 {
            return Err(anyhow!(
                "Usage: {} <audio_file> [--server-url <url>] [--sample-rate <rate>] [--channels <ch>] [--bit-depth <depth>]",
                args[0]
            ));
        }

        let mut config = ClientConfig {
            server_url: "http://localhost:8080".to_string(),
            audio_file: args[1].clone(),
            sample_rate: 16000,
            channels: 1,
            bit_depth: 16,
        };

        let mut i = 2;
        while i < args.len() {
            match args[i].as_str() {
                "--server-url" => {
                    if i + 1 < args.len() {
                        config.server_url = args[i + 1].clone();
                        i += 2;
                    } else {
                        return Err(anyhow!("--server-url requires a value"));
                    }
                }
                "--sample-rate" => {
                    if i + 1 < args.len() {
                        config.sample_rate = args[i + 1].parse()
                            .map_err(|_| anyhow!("Invalid sample rate"))?;
                        i += 2;
                    } else {
                        return Err(anyhow!("--sample-rate requires a value"));
                    }
                }
                "--channels" => {
                    if i + 1 < args.len() {
                        config.channels = args[i + 1].parse()
                            .map_err(|_| anyhow!("Invalid channels count"))?;
                        i += 2;
                    } else {
                        return Err(anyhow!("--channels requires a value"));
                    }
                }
                "--bit-depth" => {
                    if i + 1 < args.len() {
                        config.bit_depth = args[i + 1].parse()
                            .map_err(|_| anyhow!("Invalid bit depth"))?;
                        i += 2;
                    } else {
                        return Err(anyhow!("--bit-depth requires a value"));
                    }
                }
                _ => {
                    return Err(anyhow!("Unknown argument: {}", args[i]));
                }
            }
        }

        Ok(config)
    }
}

async fn send_transcription_request(config: &ClientConfig) -> Result<Value> {
    let client = reqwest::Client::new();
    
    // Check if file exists
    if !Path::new(&config.audio_file).exists() {
        return Err(anyhow!("Audio file not found: {}", config.audio_file));
    }

    // Read the audio file
    let audio_data = fs::read(&config.audio_file)
        .map_err(|e| anyhow!("Failed to read audio file: {}", e))?;

    println!("📁 Loaded audio file: {} ({} bytes)", config.audio_file, audio_data.len());

    // Create multipart form
    let form = reqwest::multipart::Form::new()
        .part("audio", reqwest::multipart::Part::bytes(audio_data)
            .file_name(config.audio_file.clone()))
        .text("sample_rate", config.sample_rate.to_string())
        .text("channels", config.channels.to_string())
        .text("bit_depth", config.bit_depth.to_string());

    println!("🚀 Sending transcription request to: {}/api/v1/transcribe", config.server_url);
    println!("   Sample rate: {}Hz, Channels: {}, Bit depth: {}", 
             config.sample_rate, config.channels, config.bit_depth);

    // Send the request
    let response = client
        .post(&format!("{}/api/v1/transcribe", config.server_url))
        .multipart(form)
        .send()
        .await
        .map_err(|e| anyhow!("Failed to send request: {}", e))?;

    let status = response.status();
    let response_text = response.text().await
        .map_err(|e| anyhow!("Failed to read response: {}", e))?;

    if !status.is_success() {
        return Err(anyhow!("Server returned error {}: {}", status, response_text));
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

    let config = ClientConfig::from_args()?;
    
    println!("🎵 Open Transcribe Client");
    println!("========================");

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