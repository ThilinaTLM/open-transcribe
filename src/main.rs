mod whisper;

use actix_multipart::{Field, Multipart};
use actix_web::{App, HttpResponse, HttpServer, Responder, get, post, web};
use futures_util::TryStreamExt;
use serde_json;
use whisper::config::WhisperConfig;
use whisper::transcriber::{InputAudio, SimpleTranscriber};

#[get("/api/v1/health")]
async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok"
    }))
}

#[derive(serde::Serialize)]
struct TranscriptionDto {
    text: String,
    segments: Option<Vec<TranscriptionSegment>>,
}

#[derive(serde::Serialize)]
struct TranscriptionSegment {
    start: usize,
    end: usize,
    text: String,
    confidence: f32,
}

#[post("/api/v1/transcribe")]
async fn transcribe_upload(mut payload: Multipart) -> impl Responder {
    let mut audio_data: Option<Vec<u8>> = None;
    let mut sample_rate: u32 = 16000;
    let mut channels: usize = 1;
    let mut bit_depth: u8 = 16;

    // Process multipart fields
    while let Some(field) = payload.try_next().await.unwrap_or(None) {
        match field.name() {
            Some("audio") => match read_field_data(field).await {
                Ok(data) => audio_data = Some(data),
                Err(_) => {
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": "Failed to read audio data"
                    }));
                }
            },
            Some("sample_rate") => {
                if let Ok(data) = read_field_data(field).await {
                    if let Ok(text) = String::from_utf8(data) {
                        sample_rate = text.trim().parse().unwrap_or(16000);
                    }
                }
            }
            Some("channels") => {
                if let Ok(data) = read_field_data(field).await {
                    if let Ok(text) = String::from_utf8(data) {
                        channels = text.trim().parse().unwrap_or(1);
                    }
                }
            }
            Some("bit_depth") => {
                if let Ok(data) = read_field_data(field).await {
                    if let Ok(text) = String::from_utf8(data) {
                        bit_depth = text.trim().parse().unwrap_or(16);
                    }
                }
            }
            _ => {
                continue;
            }
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

    let audio_samples: Vec<f32> = match bit_depth {
        16 => audio_bytes
            .chunks_exact(2)
            .map(|chunk| {
                let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                sample as f32 / i16::MAX as f32
            })
            .collect(),
        24 => audio_bytes
            .chunks_exact(3)
            .map(|chunk| {
                let sample = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], 0]) >> 8;
                sample as f32 / 8388607.0
            })
            .collect(),
        32 => audio_bytes
            .chunks_exact(4)
            .map(|chunk| {
                let sample = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                sample as f32 / i32::MAX as f32
            })
            .collect(),
        _ => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": format!("Unsupported bit depth: {}", bit_depth)
            }));
        }
    };

    transcribe_audio_samples(audio_samples, sample_rate, channels).await
}

async fn read_field_data(mut field: Field) -> Result<Vec<u8>, actix_web::Error> {
    let mut data = Vec::new();
    while let Some(chunk) = field.try_next().await? {
        data.extend_from_slice(&chunk);
    }
    Ok(data)
}

async fn transcribe_audio_samples(
    audio_samples: Vec<f32>,
    sample_rate: u32,
    channels: usize,
) -> HttpResponse {
    if audio_samples.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "No audio data provided"
        }));
    }

    let config = WhisperConfig::default();
    let transcriber = match SimpleTranscriber::new(config) {
        Ok(t) => t,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to initialize transcriber: {}", e)
            }));
        }
    };

    let input_audio = InputAudio {
        data: &audio_samples,
        sample_rate,
        channels,
    };

    match transcriber.transcribe(&input_audio) {
        Ok(output) => {
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
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Transcription failed: {}", e)
        })),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .app_data(web::JsonConfig::default().limit(50 * 1024 * 1024)) // 50MB
            .app_data(
                actix_multipart::form::MultipartFormConfig::default()
                    .total_limit(100 * 1024 * 1024), // 100MB
            )
            .service(health_check)
            .service(transcribe_upload)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
