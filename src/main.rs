mod whisper;

use actix_multipart::{Field, Multipart};
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, Responder, get, post, web};
use base64::{Engine as _, engine::general_purpose};
use futures_util::TryStreamExt;
use serde_json;
use whisper::config::WhisperConfig;
use whisper::transcriber::{InputAudio, SimpleTranscriber};

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().json(WhisperConfig::default())
}

#[derive(serde::Deserialize)]
struct TranscribeRequest {
    #[serde(default)]
    audio: Vec<u8>, // Raw bytes (deprecated)
    #[serde(default)]
    audio_base64: Option<String>, // Base64 encoded audio (preferred)
    sample_rate: Option<u32>,
    channels: Option<usize>,
    bit_depth: Option<u8>,
}

#[derive(serde::Serialize)]
struct TranscribeResponse {
    text: String,
    segments: Option<Vec<TranscribeSegment>>,
}

#[derive(serde::Serialize)]
struct TranscribeSegment {
    start: usize,
    end: usize,
    text: String,
    confidence: f32,
}

// New endpoint for raw binary audio
#[post("/transcribe/raw")]
async fn transcribe_raw(req: HttpRequest, body: web::Bytes) -> impl Responder {
    // Parse headers for audio format information
    let sample_rate = req
        .headers()
        .get("X-Sample-Rate")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(16000);

    let channels = req
        .headers()
        .get("X-Channels")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1);

    let bit_depth = req
        .headers()
        .get("X-Bit-Depth")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u8>().ok())
        .unwrap_or(16);

    if body.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "No audio data provided"
        }));
    }

    // Convert binary audio to f32 samples based on bit depth
    let audio_samples: Vec<f32> = match bit_depth {
        16 => body
            .chunks_exact(2)
            .map(|chunk| {
                let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                sample as f32 / i16::MAX as f32
            })
            .collect(),
        24 => body
            .chunks_exact(3)
            .map(|chunk| {
                // 24-bit audio (3 bytes, little endian)
                let sample = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], 0]) >> 8;
                sample as f32 / 8388607.0 // 2^23 - 1
            })
            .collect(),
        32 => body
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

// New endpoint for multipart file uploads
#[post("/transcribe/upload")]
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

    // Convert binary audio to f32 samples based on bit depth
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

// Shared transcription logic
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

    // Create WhisperConfig and SimpleTranscriber
    let config = WhisperConfig::default();
    let transcriber = match SimpleTranscriber::new(config) {
        Ok(t) => t,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Failed to initialize transcriber: {}", e)
            }));
        }
    };

    // Prepare input audio
    let input_audio = InputAudio {
        data: &audio_samples,
        sample_rate,
        channels,
    };

    // Transcribe the audio
    match transcriber.transcribe(&input_audio) {
        Ok(output) => {
            let segments: Vec<TranscribeSegment> = output
                .segments
                .into_iter()
                .map(|seg| TranscribeSegment {
                    start: seg.start,
                    end: seg.end,
                    text: seg.text,
                    confidence: seg.confidence,
                })
                .collect();

            HttpResponse::Ok().json(TranscribeResponse {
                text: output.combined,
                segments: Some(segments),
            })
        }
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Transcription failed: {}", e)
        })),
    }
}

#[post("/transcribe")]
async fn transcribe(req_body: web::Json<TranscribeRequest>) -> impl Responder {
    // Default values if not provided
    let sample_rate = req_body.sample_rate.unwrap_or(16000);
    let channels = req_body.channels.unwrap_or(1);
    let bit_depth = req_body.bit_depth.unwrap_or(16);

    // Get audio data - prefer base64 over raw bytes
    let audio_bytes = if let Some(ref base64_data) = req_body.audio_base64 {
        match general_purpose::STANDARD.decode(base64_data) {
            Ok(bytes) => bytes,
            Err(_) => {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "Invalid base64 audio data"
                }));
            }
        }
    } else if !req_body.audio.is_empty() {
        req_body.audio.clone()
    } else {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "No audio data provided (use 'audio_base64' or 'audio' field)"
        }));
    };

    // Convert binary audio to f32 samples based on bit depth
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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .app_data(web::JsonConfig::default().limit(50 * 1024 * 1024)) // 50MB
            .app_data(
                actix_multipart::form::MultipartFormConfig::default()
                    .total_limit(100 * 1024 * 1024),
            ) // 100MB
            .service(hello)
            .service(transcribe)
            .service(transcribe_raw)
            .service(transcribe_upload)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
