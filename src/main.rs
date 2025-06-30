mod models;
mod whisper;

use actix_multipart::{Field, Multipart};
use actix_web::{App, HttpResponse, HttpServer, Responder, get, middleware::Logger, post, web};
use futures_util::TryStreamExt;
use serde_json;
use whisper::config::WhisperConfig;
use whisper::transcriber::{InputAudio, SimpleTranscriber};

struct AppState {
    transcriber: SimpleTranscriber,
}

#[get("/api/v1/health")]
async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "message": "Whisper transcription service is running"
    }))
}

#[post("/api/v1/transcribe")]
async fn transcribe_upload(data: web::Data<AppState>, mut payload: Multipart) -> impl Responder {
    let mut audio_data: Option<Vec<u8>> = None;
    let mut sample_rate: u32 = 16000;
    let mut channels: usize = 1;
    let mut bit_depth: u8 = 16;

    // Process multipart fields
    while let Some(field) = payload.try_next().await.unwrap_or(None) {
        match field.name() {
            Some("audio") => match read_field_data(field).await {
                Ok(data) => audio_data = Some(data),
                Err(e) => {
                    eprintln!("Failed to read audio data: {}", e);
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": "Failed to read audio data"
                    }));
                }
            },
            Some("sample_rate") => {
                if let Ok(field_data) = read_field_data(field).await {
                    if let Ok(text) = String::from_utf8(field_data) {
                        sample_rate = text.trim().parse().unwrap_or(16000);
                    }
                }
            }
            Some("channels") => {
                if let Ok(field_data) = read_field_data(field).await {
                    if let Ok(text) = String::from_utf8(field_data) {
                        channels = text.trim().parse().unwrap_or(1);
                    }
                }
            }
            Some("bit_depth") => {
                if let Ok(field_data) = read_field_data(field).await {
                    if let Ok(text) = String::from_utf8(field_data) {
                        bit_depth = text.trim().parse().unwrap_or(16);
                    }
                }
            }
            _ => continue,
        }
    }

    let audio_bytes = match audio_data {
        Some(data) => data,
        None => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "No audio file provided"
            }));
        }
    };

    // Convert raw audio bytes to f32 samples
    let audio_samples = match convert_audio_bytes_to_samples(&audio_bytes, bit_depth) {
        Ok(samples) => samples,
        Err(error_msg) => {
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
    Ok(data)
}

fn convert_audio_bytes_to_samples(audio_bytes: &[u8], bit_depth: u8) -> Result<Vec<f32>, String> {
    match bit_depth {
        16 => {
            if audio_bytes.len() % 2 != 0 {
                return Err("Invalid 16-bit audio data: odd number of bytes".to_string());
            }
            Ok(audio_bytes
                .chunks_exact(2)
                .map(|chunk| {
                    let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                    sample as f32 / i16::MAX as f32
                })
                .collect())
        }
        24 => {
            if audio_bytes.len() % 3 != 0 {
                return Err("Invalid 24-bit audio data: byte count not divisible by 3".to_string());
            }
            Ok(audio_bytes
                .chunks_exact(3)
                .map(|chunk| {
                    let sample = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], 0]) >> 8;
                    sample as f32 / 8388607.0
                })
                .collect())
        }
        32 => {
            if audio_bytes.len() % 4 != 0 {
                return Err("Invalid 32-bit audio data: byte count not divisible by 4".to_string());
            }
            Ok(audio_bytes
                .chunks_exact(4)
                .map(|chunk| {
                    let sample = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    sample as f32 / i32::MAX as f32
                })
                .collect())
        }
        _ => Err(format!("Unsupported bit depth: {}", bit_depth)),
    }
}

async fn transcribe_audio_samples(
    transcriber: &SimpleTranscriber,
    audio_samples: Vec<f32>,
    sample_rate: u32,
    channels: usize,
) -> HttpResponse {
    if audio_samples.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "No audio data provided"
        }));
    }

    let input_audio = InputAudio {
        data: &audio_samples,
        sample_rate,
        channels,
    };

    match transcriber.transcribe(&input_audio) {
        Ok(output) => {
            let segments: Vec<models::TranscriptionSegment> = output
                .segments
                .into_iter()
                .map(|seg| models::TranscriptionSegment {
                    start: seg.start,
                    end: seg.end,
                    text: seg.text,
                    confidence: seg.confidence,
                })
                .collect();

            HttpResponse::Ok().json(models::TranscriptionDto {
                text: output.combined,
                segments: Some(segments),
            })
        }
        Err(e) => {
            eprintln!("Transcription failed: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Transcription failed: {}", e)
            }))
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    println!("Initializing Whisper transcriber...");
    let config = WhisperConfig::default();
    let transcriber = match SimpleTranscriber::new(config) {
        Ok(t) => {
            println!("Whisper transcriber initialized successfully");
            t
        }
        Err(e) => {
            eprintln!("Failed to initialize transcriber: {}", e);
            std::process::exit(1);
        }
    };

    let app_state = web::Data::new(AppState { transcriber });

    println!("Starting HTTP server on 127.0.0.1:8080");

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .app_data(web::JsonConfig::default().limit(50 * 1024 * 1024)) // 50MB
            .app_data(
                actix_multipart::form::MultipartFormConfig::default()
                    .total_limit(100 * 1024 * 1024), // 100MB
            )
            .wrap(Logger::default())
            .service(health_check)
            .service(transcribe_upload)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
