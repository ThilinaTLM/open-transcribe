mod dto;
mod whisper;

use actix_cors::Cors;
use actix_multipart::{Field, Multipart};
use actix_web::{App, HttpResponse, HttpServer, Responder, get, middleware::Logger, post, web};
use futures_util::TryStreamExt;
use log::{debug, error, info, warn};
use serde_json;
use whisper::config::WhisperConfig;
use whisper::transcriber::{InputAudio, SimpleTranscriber};

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
                },
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

    info!("Processing audio: {} bytes, {}Hz, {} channels, {} bit", 
          audio_bytes.len(), sample_rate, channels, bit_depth);

    // Convert raw audio bytes to f32 samples
    let audio_samples = match convert_audio_bytes_to_samples(&audio_bytes, bit_depth) {
        Ok(samples) => {
            debug!("Successfully converted {} bytes to {} samples", audio_bytes.len(), samples.len());
            samples
        },
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
    debug!("Converting {} bytes of {}-bit audio to samples", audio_bytes.len(), bit_depth);
    
    match bit_depth {
        16 => {
            if audio_bytes.len() % 2 != 0 {
                error!("Invalid 16-bit audio data: odd number of bytes ({})", audio_bytes.len());
                return Err("Invalid 16-bit audio data: odd number of bytes".to_string());
            }
            let samples = audio_bytes
                .chunks_exact(2)
                .map(|chunk| {
                    let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                    sample as f32 / i16::MAX as f32
                })
                .collect();
            debug!("Converted 16-bit audio to {} samples", audio_bytes.len() / 2);
            Ok(samples)
        }
        24 => {
            if audio_bytes.len() % 3 != 0 {
                error!("Invalid 24-bit audio data: byte count ({}) not divisible by 3", audio_bytes.len());
                return Err("Invalid 24-bit audio data: byte count not divisible by 3".to_string());
            }
            let samples = audio_bytes
                .chunks_exact(3)
                .map(|chunk| {
                    let sample = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], 0]) >> 8;
                    sample as f32 / 8388607.0
                })
                .collect();
            debug!("Converted 24-bit audio to {} samples", audio_bytes.len() / 3);
            Ok(samples)
        }
        32 => {
            if audio_bytes.len() % 4 != 0 {
                error!("Invalid 32-bit audio data: byte count ({}) not divisible by 4", audio_bytes.len());
                return Err("Invalid 32-bit audio data: byte count not divisible by 4".to_string());
            }
            let samples = audio_bytes
                .chunks_exact(4)
                .map(|chunk| {
                    let sample = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    sample as f32 / i32::MAX as f32
                })
                .collect();
            debug!("Converted 32-bit audio to {} samples", audio_bytes.len() / 4);
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

    info!("Starting transcription: {} samples, {}Hz, {} channels", 
          audio_samples.len(), sample_rate, channels);

    let input_audio = InputAudio {
        data: &audio_samples,
        sample_rate,
        channels,
    };

    match transcriber.transcribe(&input_audio) {
        Ok(output) => {
            info!("Transcription completed successfully: {} segments, {} characters", 
                  output.segments.len(), output.combined.len());
            
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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    info!("Starting Whisper transcription service");
    info!("Initializing Whisper transcriber...");
    
    let config = WhisperConfig::default();
    info!("Using configuration: model_path={:?}, use_gpu={}, language={}, num_threads={}", 
          config.model_path, config.use_gpu, config.language, config.num_threads);
    
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

    info!("Starting HTTP server on 127.0.0.1:8080");

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
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
