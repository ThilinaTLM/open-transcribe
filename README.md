# Open Transcribe

A high-performance HTTP API for speech transcription using OpenAI's Whisper model, built with Rust and Actix-web.

## Performance Features

- **Instance Reuse**: Single Whisper model instance shared across all requests for optimal performance
- **Thread-Safe**: Concurrent request handling with mutex-protected model access
- **Memory Efficient**: Optimized audio processing and resampling
- **Fast Startup**: Model loaded once at application startup

## Configuration

Set environment variables to customize behavior:

```bash
export WHISPER_MODEL_PATH="./models/ggml-base.en.bin"
export WHISPER_USE_GPU="true"
export WHISPER_LANGUAGE="en"
export WHISPER_AUDIO_CONTEXT="768"
export WHISPER_NO_SPEECH_THRESHOLD="0.6"
export WHISPER_NUM_THREADS="4"
```

## Quick Start

1. Install dependencies:
```bash
cargo build --release
```

2. Download a Whisper model:
```bash
mkdir -p models
curl -L https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin -o models/ggml-base.en.bin
```

3. Run the server:
```bash
cargo run --release
```

## API Endpoints

### Health Check
```
GET /api/v1/health
```

### Transcribe Audio
```
POST /api/v1/transcribe
```

**Multipart Form Data:**
- `audio` (required): Raw audio data
- `sample_rate` (optional): Audio sample rate (default: 16000)
- `channels` (optional): Number of channels (default: 1)
- `bit_depth` (optional): Bit depth - 16, 24, or 32 (default: 16)

**Response:**
```json
{
  "text": "Complete transcription text",
  "segments": [
    {
      "start": 0,
      "end": 1000,
      "text": "Hello world",
      "confidence": 0.95
    }
  ]
}
```

## Performance Improvements

This implementation includes several key optimizations:

1. **Singleton Pattern**: The Whisper model is loaded once at startup and reused for all requests
2. **Thread Safety**: Uses `Arc<Mutex<>>` to safely share the model across concurrent requests
3. **Better Error Handling**: Comprehensive error messages with proper error propagation
4. **Optimized Audio Processing**: Efficient sample conversion and resampling
5. **Resource Management**: Proper cleanup and memory management

## Example Usage

```bash
# Using curl to transcribe an audio file
curl -X POST http://localhost:8080/api/v1/transcribe \
  -F "audio=@audio.raw" \
  -F "sample_rate=16000" \
  -F "channels=1" \
  -F "bit_depth=16"
```

## Requirements

- Rust 1.70+
- CUDA toolkit (optional, for GPU acceleration)
- Whisper model file (ggml format)

# Audio Recording and Transcription

This Python script records audio from your microphone and sends it to the open-transcribe Rust API for transcription.

## Setup

1. **Install Python dependencies:**
   ```bash
   pip install -r requirements.txt
   ```

2. **Start the Rust API server:**
   ```bash
   cargo run
   ```
   The server should be running on `http://127.0.0.1:8080`

## Usage

### Basic usage (record for 5 seconds):
```bash
python record_and_transcribe.py
```

### Custom recording duration:
```bash
python record_and_transcribe.py -d 10  # Record for 10 seconds
```

### List available audio devices:
```bash
python record_and_transcribe.py --list-devices
```

### Advanced options:
```bash
python record_and_transcribe.py \
    --duration 8 \
    --sample-rate 16000 \
    --channels 1 \
    --bit-depth 16 \
    --api-url http://127.0.0.1:8080
```

## Command Line Options

- `-d, --duration`: Recording duration in seconds (default: 5)
- `-r, --sample-rate`: Sample rate in Hz (default: 16000)
- `-c, --channels`: Number of audio channels (default: 1)
- `-b, --bit-depth`: Audio bit depth - 16, 24, or 32 (default: 16)
- `--api-url`: API base URL (default: http://127.0.0.1:8080)
- `--list-devices`: List available audio devices and exit

## How it works

1. The script records audio from your default microphone using `sounddevice`
2. Converts the audio to PCM format at the specified bit depth
3. Sends the raw audio data to the `/transcribe/raw` endpoint with format headers
4. Displays the transcription result with segments and confidence scores

## Requirements

- Python 3.7+
- Working microphone
- The open-transcribe Rust API server running
- Required Python packages: `sounddevice`, `numpy`, `requests`

## Troubleshooting

- **"Recording failed"**: Check microphone permissions and that your microphone is working
- **"Cannot connect to API"**: Make sure the Rust server is running on the correct port
- **Audio device issues**: Use `--list-devices` to see available audio devices 