use dotenv::dotenv;
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
        dotenv().ok();
        Self {
            model_path: PathBuf::from(get_env_var("WHISPER_MODEL_PATH")),
            use_gpu: true,
            language: "en".to_string(),
            audio_context: 768,
            no_speech_threshold: 0.5,
            num_threads: 2,
        }
    }
}

fn get_env_var(key: &str) -> String {
    std::env::var(key).expect(&format!("{} is not set", key))
}
