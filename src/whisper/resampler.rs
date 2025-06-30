use anyhow::Result;
use rubato::{Resampler, SincFixedIn, SincInterpolationType, WindowFunction};

pub fn resample_to_16khz(
    audio_data: &[f32],
    sample_rate: u32,
    channels: usize,
) -> Result<Vec<f32>> {
    if sample_rate == 16000 {
        return Ok(audio_data.to_vec());
    }

    let frames = audio_data.len() / channels;
    if frames == 0 {
        return Err(anyhow::anyhow!("No audio frames to resample"));
    }

    let params = rubato::SincInterpolationParameters {
        sinc_len: 128,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let mut input_channels = vec![Vec::with_capacity(frames); channels];
    for frame_idx in 0..frames {
        for ch in 0..channels {
            input_channels[ch].push(audio_data[frame_idx * channels + ch]);
        }
    }

    let resample_ratio = 16000.0 / sample_rate as f64;
    let mut resampler = SincFixedIn::<f32>::new(resample_ratio, 2.0, params, frames, channels)?;

    let resampled_channels = resampler.process(&input_channels, None)?;
    let delay = resampler.output_delay();
    let expected_output_frames = (frames as f64 * resample_ratio) as usize;

    let mut output = Vec::with_capacity(expected_output_frames * channels);
    let start_frame = delay;
    let end_frame = (delay + expected_output_frames).min(resampled_channels[0].len());

    for frame_idx in start_frame..end_frame {
        for ch in 0..channels {
            output.push(resampled_channels[ch][frame_idx]);
        }
    }

    Ok(output)
}
