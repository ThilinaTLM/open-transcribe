use actix_cors::Cors;
use actix_multipart::{Field, Multipart};
use actix_web::{App, HttpResponse, HttpServer, Responder, get, middleware::Logger, post, web};
use anyhow::Result;
use futures_util::TryStreamExt;
use log::{debug, error, info, warn};

use crate::audio::convert_audio_bytes_to_samples;
use crate::whisper::config::WhisperConfig;
use crate::whisper::transcriber::{InputAudio, SimpleTranscriber};

#[derive(serde::Serialize)]
pub struct TranscriptionDto {
    pub text: String,
    pub segments: Option<Vec<TranscriptionSegment>>,
}

#[derive(serde::Serialize)]
pub struct TranscriptionSegment {
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub confidence: f32,
}

pub struct AppState {
    pub transcriber: SimpleTranscriber,
}

#[get("/api/v1/health")]
pub async fn health_check() -> impl Responder {
    debug!("Health check endpoint called");
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "message": "Whisper transcription service is running"
    }))
}

#[post("/api/v1/transcribe")]
pub async fn transcribe_upload(
    data: web::Data<AppState>,
    mut payload: Multipart,
) -> impl Responder {
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
                    error!("Failed to read audio data: {e}");
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": "Failed to read audio data"
                    }));
                }
            },
            Some("sample_rate") => {
                if let Ok(field_data) = read_field_data(field).await {
                    if let Ok(text) = String::from_utf8(field_data) {
                        sample_rate = text.trim().parse().unwrap_or(16000);
                        debug!("Sample rate set to: {sample_rate}");
                    }
                }
            }
            Some("channels") => {
                if let Ok(field_data) = read_field_data(field).await {
                    if let Ok(text) = String::from_utf8(field_data) {
                        channels = text.trim().parse().unwrap_or(1);
                        debug!("Channels set to: {channels}");
                    }
                }
            }
            Some("bit_depth") => {
                if let Ok(field_data) = read_field_data(field).await {
                    if let Ok(text) = String::from_utf8(field_data) {
                        bit_depth = text.trim().parse().unwrap_or(16);
                        debug!("Bit depth set to: {bit_depth}");
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
            error!("Failed to convert audio bytes to samples: {error_msg}");
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

            let segments: Vec<TranscriptionSegment> = output
                .segments
                .into_iter()
                .map(|seg| TranscriptionSegment {
                    start: seg.start,
                    end: seg.end,
                    text: seg.text,
                    confidence: seg.confidence,
                })
                .collect();

            HttpResponse::Ok().json(TranscriptionDto {
                text: output.combined,
                segments: Some(segments),
            })
        }
        Err(e) => {
            error!("Transcription failed: {e}");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Transcription failed: {}", e)
            }))
        }
    }
}

pub async fn run_server(host: String, port: u16) -> std::io::Result<()> {
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
            error!("Failed to initialize transcriber: {e}");
            std::process::exit(1);
        }
    };

    let app_state = web::Data::new(AppState { transcriber });

    info!("Starting HTTP server on {host}:{port}");

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
