#[derive(Debug)]
pub struct ClientConfig {
    pub server_url: String,
    pub audio_file: Option<String>,
    pub sample_rate: u32,
    pub channels: usize,
    pub bit_depth: u8,
    pub record_mode: bool,
    pub record_duration: u32,
}

impl ClientConfig {
    pub fn new_file_mode(
        server_url: String,
        audio_file: String,
        sample_rate: u32,
        channels: usize,
        bit_depth: u8,
    ) -> Self {
        Self {
            server_url,
            audio_file: Some(audio_file),
            sample_rate,
            channels,
            bit_depth,
            record_mode: false,
            record_duration: 0,
        }
    }

    pub fn new_record_mode(
        server_url: String,
        sample_rate: u32,
        channels: usize,
        bit_depth: u8,
        record_duration: u32,
    ) -> Self {
        Self {
            server_url,
            audio_file: None,
            sample_rate,
            channels,
            bit_depth,
            record_mode: true,
            record_duration,
        }
    }
}
