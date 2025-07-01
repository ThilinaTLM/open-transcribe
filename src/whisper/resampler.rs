use anyhow::Result;
use log::{debug, info, warn};
use rubato::{Resampler, SincFixedIn, SincInterpolationType, WindowFunction};

pub fn resample_to_16khz(
    audio_data: &[f32],
    sample_rate: u32,
    channels: usize,
) -> Result<Vec<f32>> {
    debug!("Resampling audio: {} samples, {}Hz -> 16kHz, {} channels", 
           audio_data.len(), sample_rate, channels);
    
    if sample_rate == 16000 {
        debug!("Audio is already at 16kHz, returning original data");
        return Ok(audio_data.to_vec());
    }

    let frames = audio_data.len() / channels;
    if frames == 0 {
        warn!("No audio frames to resample");
        return Err(anyhow::anyhow!("No audio frames to resample"));
    }

    debug!("Processing {} frames ({} samples per channel)", frames, frames);

    let params = rubato::SincInterpolationParameters {
        sinc_len: 128,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    debug!("Resampler parameters: sinc_len=128, f_cutoff=0.95, interpolation=Linear");

    let mut input_channels = vec![Vec::with_capacity(frames); channels];
    for frame_idx in 0..frames {
        for ch in 0..channels {
            input_channels[ch].push(audio_data[frame_idx * channels + ch]);
        }
    }

    debug!("Prepared {} input channels with {} samples each", channels, frames);

    let resample_ratio = 16000.0 / sample_rate as f64;
    debug!("Resample ratio: {:.6} ({}Hz -> 16kHz)", resample_ratio, sample_rate);
    
    let resampler_start = std::time::Instant::now();
    let mut resampler = SincFixedIn::<f32>::new(resample_ratio, 2.0, params, frames, channels)?;
    debug!("Created resampler in {:?}", resampler_start.elapsed());

    let process_start = std::time::Instant::now();
    let resampled_channels = resampler.process(&input_channels, None)?;
    let process_duration = process_start.elapsed();
    
    let delay = resampler.output_delay();
    let expected_output_frames = (frames as f64 * resample_ratio) as usize;

    debug!("Resampling completed in {:?}: delay={} frames, expected_output={} frames", 
           process_duration, delay, expected_output_frames);

    let mut output = Vec::with_capacity(expected_output_frames * channels);
    let start_frame = delay;
    let end_frame = (delay + expected_output_frames).min(resampled_channels[0].len());

    debug!("Extracting frames {}-{} from resampled output", start_frame, end_frame);

    for frame_idx in start_frame..end_frame {
        for ch in 0..channels {
            output.push(resampled_channels[ch][frame_idx]);
        }
    }

    let actual_output_frames = (end_frame - start_frame) * channels;
    info!("Resampling complete: {}Hz -> 16kHz, {} -> {} samples ({} frames), processed in {:?}", 
          sample_rate, audio_data.len(), actual_output_frames, end_frame - start_frame, process_duration);

    Ok(output)
}
