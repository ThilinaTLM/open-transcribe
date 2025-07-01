use anyhow::{Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use log::{debug, error};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::config::ClientConfig;

pub fn convert_audio_bytes_to_samples(
    audio_bytes: &[u8],
    bit_depth: u8,
) -> Result<Vec<f32>, String> {
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

pub fn record_audio(config: &ClientConfig) -> Result<Vec<u8>> {
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

    let mut supported_configs_range = device
        .supported_input_configs()
        .map_err(|e| anyhow!("Error querying input configs: {}", e))?;

    let supported_config = supported_configs_range
        .next()
        .ok_or_else(|| anyhow!("No supported config"))?
        .with_sample_rate(cpal::SampleRate(config.sample_rate));

    let config_cpal = supported_config.into();

    let recorded_samples = Arc::new(Mutex::new(Vec::new()));
    let recorded_samples_clone = Arc::clone(&recorded_samples);

    let stream = device.build_input_stream(
        &config_cpal,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            let mut samples = recorded_samples_clone.lock().unwrap();
            samples.extend_from_slice(data);
        },
        |err| eprintln!("Error in audio stream: {}", err),
        None,
    )?;

    stream.play()?;

    println!("🔴 Recording starting in...");
    for i in (1..=3).rev() {
        print!("   {}... ", i);
        std::io::stdout().flush().unwrap();
        std::thread::sleep(Duration::from_secs(1));
    }
    println!("🎙️  GO!");

    for remaining in (1..=config.record_duration).rev() {
        if remaining % 5 == 0 || remaining <= 3 {
            println!("   {} seconds remaining...", remaining);
        }
        std::thread::sleep(Duration::from_secs(1));
    }

    drop(stream);
    println!("⏹️  Recording stopped");

    let samples = recorded_samples.lock().unwrap();
    println!("📊 Recorded {} samples", samples.len());

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
