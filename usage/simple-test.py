# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "numpy",
#     "requests",
#     "sounddevice",
# ]
# ///

import sounddevice as sd
import numpy as np
import requests
import argparse
import sys
from typing import Optional

# Default audio parameters
DEFAULT_SAMPLE_RATE = 16000
DEFAULT_CHANNELS = 1
DEFAULT_BIT_DEPTH = 16
DEFAULT_DURATION = 5  # seconds
API_BASE_URL = "http://127.0.0.1:8080"


def record_audio(
    duration: float,
    sample_rate: int = DEFAULT_SAMPLE_RATE,
    channels: int = DEFAULT_CHANNELS,
) -> np.ndarray:
    """
    Record audio from the default microphone.

    Args:
        duration: Recording duration in seconds
        sample_rate: Sample rate in Hz
        channels: Number of audio channels

    Returns:
        NumPy array containing audio samples
    """
    print(f"Recording for {duration} seconds...")
    print("Speak now!")

    # Record audio
    audio_data = sd.rec(
        int(duration * sample_rate),
        samplerate=sample_rate,
        channels=channels,
        dtype="float32",
    )

    # Wait for recording to complete
    sd.wait()
    print("Recording complete!")

    return audio_data


def convert_to_pcm_bytes(
    audio_data: np.ndarray, bit_depth: int = DEFAULT_BIT_DEPTH
) -> bytes:
    """
    Convert float32 audio data to PCM bytes.

    Args:
        audio_data: Audio data as float32 numpy array
        bit_depth: Target bit depth (16, 24, or 32)

    Returns:
        Audio data as bytes
    """
    if bit_depth == 16:
        # Convert to 16-bit PCM
        pcm_data = (audio_data * 32767).astype(np.int16)
        return pcm_data.tobytes()
    elif bit_depth == 24:
        # Convert to 24-bit PCM
        pcm_data = (audio_data * 8388607).astype(np.int32)
        # Convert to 3-byte format
        bytes_data = bytearray()
        for sample in pcm_data.flat:
            # Take only the lower 24 bits and convert to little-endian 3 bytes
            sample_24bit = sample & 0xFFFFFF
            bytes_data.extend(sample_24bit.to_bytes(3, "little", signed=True))
        return bytes(bytes_data)
    elif bit_depth == 32:
        # Convert to 32-bit PCM
        pcm_data = (audio_data * 2147483647).astype(np.int32)
        return pcm_data.tobytes()
    else:
        raise ValueError(f"Unsupported bit depth: {bit_depth}")


def transcribe_audio(
    audio_bytes: bytes,
    sample_rate: int,
    channels: int,
    bit_depth: int,
    api_url: str = API_BASE_URL,
) -> Optional[dict]:
    """
    Send audio data to the transcription API using multipart form upload.

    Args:
        audio_bytes: Raw audio data as bytes
        sample_rate: Sample rate in Hz
        channels: Number of audio channels
        bit_depth: Audio bit depth
        api_url: Base URL of the API

    Returns:
        Transcription response as dictionary, or None if failed
    """
    url = f"{api_url}/transcribe/upload"

    # Prepare multipart form data
    files = {"audio": ("audio.pcm", audio_bytes, "application/octet-stream")}

    data = {
        "sample_rate": str(sample_rate),
        "channels": str(channels),
        "bit_depth": str(bit_depth),
    }

    try:
        print("Sending audio to transcription API via multipart upload...")
        response = requests.post(url, files=files, data=data, timeout=30)

        if response.status_code == 200:
            return response.json()
        else:
            print(f"API error: {response.status_code}")
            try:
                error_info = response.json()
                print(f"Error details: {error_info}")
            except:
                print(f"Response: {response.text}")
            return None

    except requests.RequestException as e:
        print(f"Request failed: {e}")
        return None


def main():
    parser = argparse.ArgumentParser(
        description="Record audio and transcribe using open-transcribe API"
    )
    parser.add_argument(
        "-d",
        "--duration",
        type=float,
        default=DEFAULT_DURATION,
        help=f"Recording duration in seconds (default: {DEFAULT_DURATION})",
    )
    parser.add_argument(
        "-r",
        "--sample-rate",
        type=int,
        default=DEFAULT_SAMPLE_RATE,
        help=f"Sample rate in Hz (default: {DEFAULT_SAMPLE_RATE})",
    )
    parser.add_argument(
        "-c",
        "--channels",
        type=int,
        default=DEFAULT_CHANNELS,
        help=f"Number of channels (default: {DEFAULT_CHANNELS})",
    )
    parser.add_argument(
        "-b",
        "--bit-depth",
        type=int,
        choices=[16, 24, 32],
        default=DEFAULT_BIT_DEPTH,
        help=f"Bit depth (default: {DEFAULT_BIT_DEPTH})",
    )
    parser.add_argument(
        "--api-url",
        default=API_BASE_URL,
        help=f"API base URL (default: {API_BASE_URL})",
    )
    parser.add_argument(
        "--list-devices",
        action="store_true",
        help="List available audio devices and exit",
    )

    args = parser.parse_args()

    if args.list_devices:
        print("Available audio devices:")
        print(sd.query_devices())
        return

    try:
        # Test API connection
        test_url = f"{args.api_url}/"
        print(f"Testing API connection to {test_url}...")
        response = requests.get(test_url, timeout=5)
        if response.status_code == 200:
            print("✓ API is accessible")
        else:
            print(f"⚠ API returned status {response.status_code}")
    except requests.RequestException as e:
        print(f"✗ Cannot connect to API: {e}")
        print("Make sure the Rust server is running on the specified URL")
        sys.exit(1)

    # Record audio
    try:
        audio_data = record_audio(args.duration, args.sample_rate, args.channels)
    except Exception as e:
        print(f"Recording failed: {e}")
        print("Make sure you have a working microphone and the required permissions")
        sys.exit(1)

    # Convert to PCM bytes
    try:
        audio_bytes = convert_to_pcm_bytes(audio_data, args.bit_depth)
        print(f"Audio converted: {len(audio_bytes)} bytes")
    except Exception as e:
        print(f"Audio conversion failed: {e}")
        sys.exit(1)

    # Transcribe
    result = transcribe_audio(
        audio_bytes, args.sample_rate, args.channels, args.bit_depth, args.api_url
    )

    if result:
        print("\n" + "=" * 50)
        print("TRANSCRIPTION RESULT:")
        print("=" * 50)
        print(f"Text: {result['text']}")

        if result.get("segments"):
            print("\nSegments:")
            for i, segment in enumerate(result["segments"]):
                print(
                    f"  {i+1}. [{segment['start']}-{segment['end']}] "
                    f"(confidence: {segment['confidence']:.2f}) {segment['text']}"
                )
    else:
        print("Transcription failed!")
        sys.exit(1)


if __name__ == "__main__":
    main()
