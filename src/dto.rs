#[derive(serde::Serialize)]
pub struct TranscriptionDto {
    pub text: String,
    pub segments: Option<Vec<TranscriptionSegment>>,
}

#[derive(serde::Serialize)]
pub struct TranscriptionSegment {
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub confidence: f32,
}
