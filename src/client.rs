use anyhow::{Result, anyhow};
use serde_json::Value;
use std::fs;
use std::path::Path;

use crate::audio::record_audio;
use crate::config::ClientConfig;

pub async fn send_transcription_request(config: &ClientConfig) -> Result<Value> {
    let client = reqwest::Client::new();

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

    let response = client
        .post(format!("{}/api/v1/transcribe", config.server_url))
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

pub async fn check_server_health(server_url: &str) -> Result<()> {
    let client = reqwest::Client::new();

    println!("🔍 Checking server health at: {server_url}/api/v1/health");

    let response = client
        .get(format!("{server_url}/api/v1/health"))
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

pub async fn run_client(config: ClientConfig) -> Result<()> {
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

    if let Err(e) = check_server_health(&config.server_url).await {
        eprintln!("❌ {e}");
        eprintln!("💡 Make sure the server is running: open-transcribe serve");
        return Err(e);
    }

    match send_transcription_request(&config).await {
        Ok(result) => {
            println!("\n✅ Transcription completed!");
            println!("📝 Result:");
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Err(e) => {
            eprintln!("❌ Transcription failed: {e}");
            return Err(e);
        }
    }

    Ok(())
}
