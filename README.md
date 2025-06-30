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