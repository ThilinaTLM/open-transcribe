#![allow(dead_code)]

use log::{debug, info, warn};
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
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
        debug!("Creating default WhisperConfig from environment variables");

        let model_path = std::env::var("WHISPER_MODEL_PATH").unwrap_or_else(|_| {
            debug!("WHISPER_MODEL_PATH not set, using default: ./models/ggml-base.en.bin");
            "./models/ggml-base.en.bin".to_string()
        });

        let use_gpu = std::env::var("WHISPER_USE_GPU")
            .map(|v| {
                let gpu_enabled = v.parse().unwrap_or(true);
                debug!("WHISPER_USE_GPU={v}, parsed as: {gpu_enabled}");
                gpu_enabled
            })
            .unwrap_or_else(|_| {
                debug!("WHISPER_USE_GPU not set, defaulting to: true");
                true
            });

        let language = std::env::var("WHISPER_LANGUAGE").unwrap_or_else(|_| {
            debug!("WHISPER_LANGUAGE not set, defaulting to: en");
            "en".to_string()
        });

        let audio_context = std::env::var("WHISPER_AUDIO_CONTEXT")
            .map(|v| {
                let context = v.parse().unwrap_or(768);
                debug!("WHISPER_AUDIO_CONTEXT={v}, parsed as: {context}");
                context
            })
            .unwrap_or_else(|_| {
                debug!("WHISPER_AUDIO_CONTEXT not set, defaulting to: 768");
                768
            });

        let no_speech_threshold = std::env::var("WHISPER_NO_SPEECH_THRESHOLD")
            .map(|v| {
                let threshold = v.parse().unwrap_or(0.6);
                debug!(
                    "WHISPER_NO_SPEECH_THRESHOLD={v}, parsed as: {threshold}"
                );
                threshold
            })
            .unwrap_or_else(|_| {
                debug!("WHISPER_NO_SPEECH_THRESHOLD not set, defaulting to: 0.6");
                0.6
            });

        let default_threads = std::thread::available_parallelism()
            .map(|n| n.get() as i32)
            .unwrap_or(4);

        let num_threads = std::env::var("WHISPER_NUM_THREADS")
            .map(|v| {
                let threads = v.parse().unwrap_or(default_threads);
                debug!("WHISPER_NUM_THREADS={v}, parsed as: {threads}");
                threads
            })
            .unwrap_or_else(|_| {
                debug!(
                    "WHISPER_NUM_THREADS not set, defaulting to: {default_threads} (available parallelism)"
                );
                default_threads
            });

        let config = Self {
            model_path: PathBuf::from(model_path),
            use_gpu,
            language,
            audio_context,
            no_speech_threshold,
            num_threads,
        };

        // Validate configuration
        if !config.model_path.exists() {
            warn!("Model path does not exist: {:?}", config.model_path);
        }

        if config.audio_context < 1 || config.audio_context > 4096 {
            warn!(
                "Audio context {} is outside recommended range (1-4096)",
                config.audio_context
            );
        }

        if config.no_speech_threshold < 0.0 || config.no_speech_threshold > 1.0 {
            warn!(
                "No speech threshold {} is outside valid range (0.0-1.0)",
                config.no_speech_threshold
            );
        }

        if config.num_threads < 1 {
            warn!(
                "Number of threads {} is invalid, should be >= 1",
                config.num_threads
            );
        }

        info!(
            "WhisperConfig created: model_path={:?}, use_gpu={}, language={}, audio_context={}, no_speech_threshold={}, num_threads={}",
            config.model_path,
            config.use_gpu,
            config.language,
            config.audio_context,
            config.no_speech_threshold,
            config.num_threads
        );

        config
    }
}

impl WhisperConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_model_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.model_path = path.into();
        info!("Updated model path to: {:?}", self.model_path);
        self
    }

    pub fn with_language<S: Into<String>>(mut self, language: S) -> Self {
        self.language = language.into();
        info!("Updated language to: {}", self.language);
        self
    }

    pub fn with_gpu(mut self, use_gpu: bool) -> Self {
        self.use_gpu = use_gpu;
        info!("Updated GPU usage to: {}", self.use_gpu);
        self
    }
}
