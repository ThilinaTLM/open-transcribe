import axios, { type AxiosInstance, type AxiosResponse } from "axios";

const API_BASE_URL = "http://127.0.0.1:8080";

export type TranscriptionSegmentDto = {
  start: number;
  end: number;
  text: string;
  confidence: number;
};

export type TranscriptionDto = {
  text: string;
  segments?: TranscriptionSegmentDto[];
};

export type HealthCheckDto = {
  status: string;
  message: string;
};

export type TranscribeFormDto = {
  audio: File | Blob;
  sampleRate?: number;
  channels?: number;
  bitDepth?: 16 | 24 | 32;
};

export class TranscriptionApiClient {
  private client: AxiosInstance;

  constructor(baseURL: string = API_BASE_URL) {
    this.client = axios.create({
      baseURL,
      timeout: 120000, // 2 minutes
      headers: {
        Accept: "application/json",
      },
    });

    this.client.interceptors.response.use(
      (response) => response,
      (error) => {
        if (error.response?.data?.error) {
          throw new Error(error.response.data.error);
        }
        throw error;
      }
    );
  }

  async healthCheck(): Promise<HealthCheckDto> {
    try {
      const response: AxiosResponse<HealthCheckDto> = await this.client.get(
        "/api/v1/health"
      );
      return response.data;
    } catch (error) {
      throw new Error(
        `Health check failed: ${
          error instanceof Error ? error.message : "Unknown error"
        }`
      );
    }
  }

  async transcribe(request: TranscribeFormDto): Promise<TranscriptionDto> {
    try {
      const formData = new FormData();

      formData.append("audio", request.audio);

      if (request.sampleRate !== undefined) {
        formData.append("sample_rate", request.sampleRate.toString());
      }

      if (request.channels !== undefined) {
        formData.append("channels", request.channels.toString());
      }

      if (request.bitDepth !== undefined) {
        formData.append("bit_depth", request.bitDepth.toString());
      }

      const response: AxiosResponse<TranscriptionDto> = await this.client.post(
        "/api/v1/transcribe",
        formData,
        {
          headers: {
            "Content-Type": "multipart/form-data",
          },
        }
      );

      return response.data;
    } catch (error) {
      throw new Error(
        `Transcription failed: ${
          error instanceof Error ? error.message : "Unknown error"
        }`
      );
    }
  }

  async transcribeFile(
    file: File,
    options: Omit<TranscribeFormDto, "audio"> = {}
  ): Promise<TranscriptionDto> {
    return this.transcribe({
      audio: file,
      ...options,
    });
  }

  async transcribeRawAudio(
    audioData: ArrayBuffer | Uint8Array,
    options: Omit<TranscribeFormDto, "audio"> = {}
  ): Promise<TranscriptionDto> {
    const blob = new Blob([audioData], { type: "application/octet-stream" });
    return this.transcribe({
      audio: blob,
      ...options,
    });
  }

  setBaseURL(baseURL: string): void {
    this.client.defaults.baseURL = baseURL;
  }

  setTimeout(timeout: number): void {
    this.client.defaults.timeout = timeout;
  }
}

export default new TranscriptionApiClient();
