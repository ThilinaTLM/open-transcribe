#![allow(dead_code)]

use std::path::PathBuf;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct WhisperConfig {
    pub model_path: PathBuf,
    pub use_gpu: bool,
    pub language: String,
    pub audio_context: i32,
    pub no_speech_threshold: f32,
    pub num_threads: i32,
}

impl Default for WhisperConfig {
    fn default() -> Self {
        Self {
            model_path: PathBuf::from(
                std::env::var("WHISPER_MODEL_PATH")
                    .unwrap_or_else(|_| "./models/ggml-base.en.bin".to_string())
            ),
            use_gpu: std::env::var("WHISPER_USE_GPU")
                .map(|v| v.parse().unwrap_or(true))
                .unwrap_or(true),
            language: std::env::var("WHISPER_LANGUAGE")
                .unwrap_or_else(|_| "en".to_string()),
            audio_context: std::env::var("WHISPER_AUDIO_CONTEXT")
                .map(|v| v.parse().unwrap_or(768))
                .unwrap_or(768),
            no_speech_threshold: std::env::var("WHISPER_NO_SPEECH_THRESHOLD")
                .map(|v| v.parse().unwrap_or(0.6))
                .unwrap_or(0.6),
            num_threads: std::env::var("WHISPER_NUM_THREADS")
                .map(|v| v.parse().unwrap_or(std::thread::available_parallelism().map(|n| n.get() as i32).unwrap_or(4)))
                .unwrap_or_else(|_| std::thread::available_parallelism().map(|n| n.get() as i32).unwrap_or(4)),
        }
    }
}

impl WhisperConfig {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn with_model_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.model_path = path.into();
        self
    }
    
    pub fn with_language<S: Into<String>>(mut self, language: S) -> Self {
        self.language = language.into();
        self
    }
    
    pub fn with_gpu(mut self, use_gpu: bool) -> Self {
        self.use_gpu = use_gpu;
        self
    }
}
