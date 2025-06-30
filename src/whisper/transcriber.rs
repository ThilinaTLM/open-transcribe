#![allow(dead_code)]

use anyhow::Result;
use std::sync::{Arc, Mutex};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::whisper::config::WhisperConfig;

pub struct InputAudio<'a> {
    pub data: &'a [f32],
    pub sample_rate: u32,
    pub channels: usize,
}

pub struct TranscribeOutput {
    pub combined: String,
    pub segments: Vec<Segment>,
}

#[derive(Clone)]
pub struct Segment {
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub confidence: f32,
}

impl PartialEq for Segment {
    fn eq(&self, other: &Self) -> bool {
        self.start == other.start && self.end == other.end && self.text == other.text
    }
}

#[derive(Clone)]
pub struct SimpleTranscriber {
    inner: Arc<Mutex<TranscriberInner>>,
    config: WhisperConfig,
}

struct TranscriberInner {
    ctx: WhisperContext,
}

impl SimpleTranscriber {
    pub fn new(config: WhisperConfig) -> Result<Self> {
        let mut ctx_params = WhisperContextParameters::default();
        ctx_params.use_gpu(config.use_gpu);

        let ctx = WhisperContext::new_with_params(
            config.model_path.to_str().unwrap(), 
            ctx_params
        ).map_err(|e| anyhow::anyhow!("Failed to load model: {}", e))?;

        let inner = TranscriberInner { ctx };

        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
            config,
        })
    }

    pub fn transcribe(&self, audio_data: &InputAudio) -> Result<TranscribeOutput> {
        // Resample audio to 16kHz if needed
        let resampled_audio = crate::whisper::resampler::resample_to_16khz(
            audio_data.data,
            audio_data.sample_rate,
            audio_data.channels,
        )?;

        if resampled_audio.len() < 16000 {
            return Err(anyhow::anyhow!("Audio is too short (less than 1 second)"));
        }

        // Convert to mono
        let mono_audio = whisper_rs::convert_stereo_to_mono_audio(&resampled_audio)
            .map_err(|e| anyhow::anyhow!("Failed to convert audio to mono: {}", e))?;

        // Configure transcription parameters
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(&self.config.language));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(true);
        params.set_audio_ctx(self.config.audio_context);
        params.set_no_speech_thold(self.config.no_speech_threshold);
        params.set_n_threads(self.config.num_threads);

        // Lock the context and run transcription
        let inner = self.inner.lock()
            .map_err(|_| anyhow::anyhow!("Failed to acquire transcriber lock"))?;

        let mut state = inner.ctx.create_state()
            .map_err(|e| anyhow::anyhow!("Failed to create whisper state: {}", e))?;

        state.full(params, &mono_audio)
            .map_err(|e| anyhow::anyhow!("Failed to run transcription: {}", e))?;

        // Extract results
        let num_segments = state.full_n_segments()
            .map_err(|e| anyhow::anyhow!("Failed to get segment count: {}", e))?;

        let mut combined = String::new();
        let mut segments = Vec::with_capacity(num_segments as usize);

        for i in 0..num_segments {
            let text = state.full_get_segment_text(i)
                .map_err(|e| anyhow::anyhow!("Failed to get segment text: {}", e))?;
            
            let start = state.full_get_segment_t0(i)
                .map_err(|e| anyhow::anyhow!("Failed to get segment start: {}", e))?;
            
            let end = state.full_get_segment_t1(i)
                .map_err(|e| anyhow::anyhow!("Failed to get segment end: {}", e))?;

            // Calculate confidence from token probabilities
            let confidence = self.calculate_segment_confidence(&state, i)?;

            combined.push_str(&text);
            segments.push(Segment {
                start: start as usize,
                end: end as usize,
                text,
                confidence,
            });
        }

        Ok(TranscribeOutput { combined, segments })
    }

    fn calculate_segment_confidence(&self, state: &whisper_rs::WhisperState, segment_idx: i32) -> Result<f32> {
        let n_tokens = state.full_n_tokens(segment_idx)?;
        if n_tokens == 0 {
            return Ok(0.0);
        }

        let mut sum_logprob = 0.0_f32;
        for token_idx in 0..n_tokens {
            let token_data = state.full_get_token_data(segment_idx, token_idx)?;
            sum_logprob += token_data.plog;
        }

        let avg_logprob = sum_logprob / n_tokens as f32;
        Ok(avg_logprob.exp())
    }
}
