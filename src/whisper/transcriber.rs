#![allow(dead_code)]

use anyhow::Result;
use log::{debug, error, info, warn};
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
        info!(
            "Creating new SimpleTranscriber with model: {:?}",
            config.model_path
        );

        let mut ctx_params = WhisperContextParameters::default();
        ctx_params.use_gpu(config.use_gpu);

        debug!("Whisper context parameters: use_gpu={}", config.use_gpu);

        let ctx = WhisperContext::new_with_params(config.model_path.to_str().unwrap(), ctx_params)
            .map_err(|e| {
                error!(
                    "Failed to load Whisper model from {:?}: {}",
                    config.model_path, e
                );
                anyhow::anyhow!("Failed to load model: {}", e)
            })?;

        info!(
            "Whisper model loaded successfully from {:?}",
            config.model_path
        );

        let inner = TranscriberInner { ctx };

        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
            config,
        })
    }

    pub fn transcribe(&self, audio_data: &InputAudio) -> Result<TranscribeOutput> {
        let start_time = std::time::Instant::now();

        debug!(
            "Starting transcription: {} samples, {}Hz, {} channels",
            audio_data.data.len(),
            audio_data.sample_rate,
            audio_data.channels
        );

        // Resample audio to 16kHz if needed
        let resampled_audio = if audio_data.sample_rate != 16000 {
            info!(
                "Resampling audio from {}Hz to 16kHz",
                audio_data.sample_rate
            );
            crate::whisper::resampler::resample_to_16khz(
                audio_data.data,
                audio_data.sample_rate,
                audio_data.channels,
            )?
        } else {
            debug!("Audio already at 16kHz, skipping resampling");
            audio_data.data.to_vec()
        };

        debug!("Audio after resampling: {} samples", resampled_audio.len());

        if resampled_audio.len() < 16000 {
            warn!(
                "Audio is too short: {} samples (less than 1 second)",
                resampled_audio.len()
            );
            return Err(anyhow::anyhow!("Audio is too short (less than 1 second)"));
        }

        // Convert to mono
        debug!("Converting stereo audio to mono");
        let mono_audio =
            whisper_rs::convert_stereo_to_mono_audio(&resampled_audio).map_err(|e| {
                error!("Failed to convert audio to mono: {e}");
                anyhow::anyhow!("Failed to convert audio to mono: {e}")
            })?;

        debug!("Audio converted to mono: {} samples", mono_audio.len());

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

        debug!(
            "Transcription parameters: language={}, audio_ctx={}, no_speech_threshold={}, threads={}",
            self.config.language,
            self.config.audio_context,
            self.config.no_speech_threshold,
            self.config.num_threads
        );

        // Lock the context and run transcription
        let inner = self.inner.lock().map_err(|_| {
            error!("Failed to acquire transcriber lock");
            anyhow::anyhow!("Failed to acquire transcriber lock")
        })?;

        debug!("Acquired transcriber lock, creating whisper state");
        let mut state = inner.ctx.create_state().map_err(|e| {
            error!("Failed to create whisper state: {e}");
            anyhow::anyhow!("Failed to create whisper state: {e}")
        })?;

        debug!(
            "Running whisper transcription on {} samples",
            mono_audio.len()
        );
        let transcription_start = std::time::Instant::now();

        state.full(params, &mono_audio).map_err(|e| {
            error!("Failed to run transcription: {e}");
            anyhow::anyhow!("Failed to run transcription: {e}")
        })?;

        let transcription_duration = transcription_start.elapsed();
        info!("Whisper transcription completed in {transcription_duration:?}");

        // Extract results
        let num_segments = state.full_n_segments().map_err(|e| {
            error!("Failed to get segment count: {e}");
            anyhow::anyhow!("Failed to get segment count: {e}")
        })?;

        debug!("Extracting {num_segments} segments from transcription result");

        let mut combined = String::new();
        let mut segments = Vec::with_capacity(num_segments as usize);

        for i in 0..num_segments {
            let text = state.full_get_segment_text(i).map_err(|e| {
                error!("Failed to get segment {i} text: {e}");
                anyhow::anyhow!("Failed to get segment text: {e}")
            })?;

            let start = state.full_get_segment_t0(i).map_err(|e| {
                error!("Failed to get segment {i} start time: {e}");
                anyhow::anyhow!("Failed to get segment start: {e}")
            })?;

            let end = state.full_get_segment_t1(i).map_err(|e| {
                error!("Failed to get segment {i} end time: {e}");
                anyhow::anyhow!("Failed to get segment end: {e}")
            })?;

            // Calculate confidence from token probabilities
            let confidence = self.calculate_segment_confidence(&state, i)?;

            debug!(
                "Segment {}: {}ms-{}ms, confidence: {:.3}, text: {:?}",
                i,
                start,
                end,
                confidence,
                text.trim()
            );

            combined.push_str(&text);
            segments.push(Segment {
                start: start as usize,
                end: end as usize,
                text,
                confidence,
            });
        }

        let total_duration = start_time.elapsed();
        let audio_duration_seconds = mono_audio.len() as f64 / 16000.0;
        let real_time_factor = audio_duration_seconds / total_duration.as_secs_f64();

        info!(
            "Transcription complete: {} segments, {} characters, {:.1}s audio processed in {:?} (RTF: {:.2}x)",
            segments.len(),
            combined.len(),
            audio_duration_seconds,
            total_duration,
            real_time_factor
        );

        Ok(TranscribeOutput { combined, segments })
    }

    fn calculate_segment_confidence(
        &self,
        state: &whisper_rs::WhisperState,
        segment_idx: i32,
    ) -> Result<f32> {
        let n_tokens = state.full_n_tokens(segment_idx)?;
        if n_tokens == 0 {
            debug!("Segment {segment_idx} has no tokens, returning confidence 0.0");
            return Ok(0.0);
        }

        let mut sum_logprob = 0.0_f32;
        for token_idx in 0..n_tokens {
            let token_data = state.full_get_token_data(segment_idx, token_idx)?;
            sum_logprob += token_data.plog;
        }

        let avg_logprob = sum_logprob / n_tokens as f32;
        let confidence = avg_logprob.exp();

        debug!(
            "Segment {segment_idx} confidence calculation: {n_tokens} tokens, avg_logprob: {avg_logprob:.4}, confidence: {confidence:.4}"
        );

        Ok(confidence)
    }
}
