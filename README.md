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

# Client Application

The client application supports both file transcription and live audio recording using the native Rust [cpal](https://github.com/RustAudio/cpal) library for cross-platform audio I/O.

## Features

- **File Mode**: Transcribe existing audio files
- **Recording Mode**: Record audio directly from your microphone and transcribe it
- **Cross-platform**: Works on Linux, Windows, macOS, and more
- **Flexible Audio Settings**: Configurable sample rate, channels, and bit depth
- **Real-time Feedback**: Recording countdown and progress indicators

## Usage

### File Transcription
```bash
# Transcribe an existing audio file
cargo run --bin client audio.wav

# With custom server URL
cargo run --bin client audio.wav --server-url http://192.168.1.100:8080
```

### Audio Recording & Transcription
```bash
# Record 5 seconds (default) and transcribe
cargo run --bin client --record-mode

# Record for 10 seconds
cargo run --bin client --record-mode --record-duration 10

# Record with high-quality settings
cargo run --bin client --record-mode \
    --sample-rate 44100 \
    --channels 2 \
    --bit-depth 24 \
    --record-duration 15
```

### Command Line Options

```
USAGE:
    client [OPTIONS] [AUDIO_FILE]

MODES:
    File Mode:    client audio.wav
    Record Mode:  client --record-mode

OPTIONS:
    --record-mode                Enable audio recording mode
    --record-duration <seconds>  Recording duration (default: 5)
    --server-url <url>           Server URL (default: http://localhost:8080)
    --sample-rate <rate>         Audio sample rate (default: 16000)
    --channels <count>           Number of audio channels (default: 1)
    --bit-depth <depth>          Audio bit depth: 16, 24, or 32 (default: 16)
    --help, -h                   Show help message
```

## Recording Process

When using recording mode, the client will:

1. **Initialize Audio Device**: Automatically detect and use your default microphone
2. **Setup & Countdown**: Display recording parameters and provide a 3-second countdown
3. **Record Audio**: Capture audio with real-time progress updates
4. **Process & Send**: Convert audio to the specified format and send to server
5. **Display Results**: Show the transcription with segments and confidence scores

## Audio Format Support

- **Sample Rates**: Any rate supported by your audio device (commonly 8kHz to 192kHz)
- **Channels**: Mono (1) or Stereo (2) 
- **Bit Depths**: 16-bit, 24-bit, or 32-bit PCM
- **Input Devices**: Automatic detection of default microphone

## Requirements

- Working microphone for recording mode
- Audio drivers (ALSA on Linux, WASAPI on Windows, CoreAudio on macOS)
- The open-transcribe server running and accessible

## Troubleshooting

- **"No input device available"**: Check that your microphone is connected and recognized by the system
- **"Cannot connect to server"**: Ensure the server is running with `cargo run --bin server`
- **Permission errors**: On some systems, microphone access may require additional permissions
- **Audio quality issues**: Try adjusting sample rate and bit depth settings for your hardware

## Complete Workflow Example

Here's a complete example of using the audio recording feature:

```bash
# 1. Start the server in one terminal
cargo run --bin server

# 2. In another terminal, record and transcribe audio
cargo run --bin client --record-mode --record-duration 10

# Example output:
# 🎵 Open Transcribe Client
# ========================
# 🎤 Recording Mode
#    Duration: 10 seconds
#    Audio format: 16000Hz, 1 channels, 16-bit
#    Make sure your microphone is connected and working!
# 
# 🔍 Checking server health at: http://localhost:8080/api/v1/health
# ✅ Server is healthy
# 🎤 Starting audio recording...
#    Duration: 10 seconds
#    Sample rate: 16000Hz
#    Channels: 1
#    Bit depth: 16
# 🎙️  Using input device: Default
# 🔴 Recording starting in...
#    3... 2... 1... 🎙️  GO!
#    10 seconds remaining...
#    5 seconds remaining...
#    3 seconds remaining...
#    2 seconds remaining...
#    1 seconds remaining...
# ⏹️  Recording stopped
# 📊 Recorded 160000 samples
# 💾 Converted to 320000 bytes
# 📁 Audio source: recorded audio (320000 bytes)
# 🚀 Sending transcription request to: http://localhost:8080/api/v1/transcribe
#    Sample rate: 16000Hz, Channels: 1, Bit depth: 16
# 
# ✅ Transcription completed!
# 📝 Result:
# {
#   "text": "Hello, this is a test of the audio recording feature.",
#   "segments": [
#     {
#       "start": 0,
#       "end": 3500,
#       "text": "Hello, this is a test of the audio recording feature.",
#       "confidence": 0.92
#     }
#   ]
# }
``` 