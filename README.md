# Open Transcribe

A high-performance audio recording and transcription tool using OpenAI's Whisper model, built with Rust. Open Transcribe provides both server and client functionality in a single unified CLI application.

## Features

### Server Mode

- **Instance Reuse**: Single Whisper model instance shared across all requests for optimal performance
- **Thread-Safe**: Concurrent request handling with mutex-protected model access
- **Memory Efficient**: Optimized audio processing and resampling
- **Fast Startup**: Model loaded once at application startup
- **RESTful API**: HTTP endpoints for health checks and transcription

### Client Mode

- **File Transcription**: Process existing audio files
- **Live Recording**: Record audio directly from your microphone and transcribe
- **Cross-platform Audio**: Works on Linux, Windows, macOS using the [cpal](https://github.com/RustAudio/cpal) library
- **Flexible Audio Settings**: Configurable sample rate, channels, and bit depth
- **Real-time Feedback**: Recording countdown and progress indicators

### Model Management

- **Easy Downloads**: Built-in model downloader for Whisper models
- **Multiple Model Sizes**: Support for tiny, base, small, medium, large-v3 models
- **Flexible Storage**: Choose where to store downloaded models

## Configuration

Configure Whisper behavior using environment variables:

```bash
export WHISPER_MODEL_PATH="./models/ggml-base.en.bin"
export WHISPER_USE_GPU="true"
export WHISPER_LANGUAGE="en"
export WHISPER_AUDIO_CONTEXT="768"
export WHISPER_NO_SPEECH_THRESHOLD="0.6"
export WHISPER_NUM_THREADS="4"
```

**Configuration Options:**

- `WHISPER_MODEL_PATH`: Path to the Whisper model file (default: `./models/ggml-base.en.bin`)
- `WHISPER_USE_GPU`: Enable GPU acceleration if available (default: `true`)
- `WHISPER_LANGUAGE`: Target language code (default: `en`)
- `WHISPER_AUDIO_CONTEXT`: Audio context window size (default: `768`)
- `WHISPER_NO_SPEECH_THRESHOLD`: Threshold for detecting speech vs silence (default: `0.6`)
- `WHISPER_NUM_THREADS`: Number of threads to use (default: auto-detected)

## Installation

1. **Build from source:**

```bash
git clone https://github.com/your-username/open-transcribe
cd open-transcribe
cargo build --release
```

2. **Download a Whisper model:**

```bash
# Download base English model to ./models directory
open-transcribe download base

# Download to specific directory
open-transcribe download base ./my-models

# Available models: tiny, base, small, medium, large-v3
open-transcribe download large-v3
```

## Usage

Open Transcribe provides a unified CLI with multiple subcommands:

### Start the Server

```bash
# Start server on default host/port (127.0.0.1:8080)
open-transcribe serve

# Custom host and port
open-transcribe serve --host 0.0.0.0 --port 9000
```

### Download Models

```bash
# Download base model to current directory
open-transcribe download base

# Download to specific directory
open-transcribe download large-v3 ./models

# Available models: tiny, base, small, medium, large-v3
```

### Transcribe Audio Files

```bash
# Transcribe existing audio file (server must be running)
open-transcribe file audio.wav

# Use custom server URL
open-transcribe file audio.wav --server-url http://192.168.1.100:8080

# Specify audio format details
open-transcribe file audio.wav --sample-rate 44100 --channels 2 --bit-depth 24
```

### Record and Transcribe

```bash
# Record 5 seconds (default) and transcribe
open-transcribe record

# Record for 10 seconds
open-transcribe record --duration 10

# Record with high-quality settings
open-transcribe record --duration 15 --sample-rate 44100 --channels 2 --bit-depth 24

# Use custom server
open-transcribe record --server-url http://my-server:8080
```

## API Endpoints

When running in server mode, the following HTTP endpoints are available:

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

**Example using curl:**

```bash
curl -X POST http://localhost:8080/api/v1/transcribe \
  -F "audio=@audio.raw" \
  -F "sample_rate=16000" \
  -F "channels=1" \
  -F "bit_depth=16"
```

## Complete Workflow Example

Here's a complete example from setup to transcription:

```bash
# 1. Build the application
cargo build --release

# 2. Download a model
open-transcribe download base

# 3. Start the server in one terminal
open-transcribe serve

# 4. In another terminal, record and transcribe audio
open-transcribe record --duration 10

# Example output:
# 🎵 Open Transcribe
# ==================
# 🎤 Recording for 10 seconds...
# 🔴 Recording starting in...
#    3... 2... 1... 🎙️  GO!
#    10 seconds remaining...
#    5 seconds remaining...
#    Recording complete!
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

## Audio Format Support

- **Sample Rates**: Any rate supported by your audio device (commonly 8kHz to 192kHz)
- **Channels**: Mono (1) or Stereo (2)
- **Bit Depths**: 16-bit, 24-bit, or 32-bit PCM
- **Input Devices**: Automatic detection of default microphone

## Performance Optimizations

This implementation includes several key optimizations:

1. **Singleton Pattern**: The Whisper model is loaded once at startup and reused for all requests
2. **Thread Safety**: Uses `Arc<Mutex<>>` to safely share the model across concurrent requests
3. **Comprehensive Error Handling**: Proper error messages with detailed error propagation
4. **Optimized Audio Processing**: Efficient sample conversion and resampling
5. **Resource Management**: Proper cleanup and memory management
6. **Environment-based Configuration**: Flexible configuration through environment variables

## Requirements

- **Rust**: 1.70+ (2024 edition)
- **Audio System**: ALSA (Linux), WASAPI (Windows), CoreAudio (macOS)
- **CUDA Toolkit**: Optional, for GPU acceleration
- **Whisper Model**: Download using the built-in `download` command

## Troubleshooting

### Recording Issues

- **"No input device available"**: Check that your microphone is connected and recognized by the system
- **Permission errors**: On some systems, microphone access may require additional permissions
- **Audio quality issues**: Try adjusting sample rate and bit depth settings for your hardware

### Server Issues

- **"Cannot connect to server"**: Ensure the server is running with `open-transcribe serve`
- **Model loading errors**: Verify the model path exists and is accessible
- **GPU issues**: Disable GPU with `WHISPER_USE_GPU=false` if experiencing CUDA problems

### Model Issues

- **Download failures**: Check internet connection and try different model sizes
- **Model path errors**: Ensure the downloaded model path matches `WHISPER_MODEL_PATH`

## Development

To contribute to Open Transcribe:

```bash
# Clone and build
git clone https://github.com/your-username/open-transcribe
cd open-transcribe
cargo build

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run -- serve
```

## License

[Add your license information here]
