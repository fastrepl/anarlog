import {
  Directory,
  File,
  FileMode,
  Paths,
  type FileHandle,
} from "expo-file-system";

import { wavHeader } from "@/audio/pcm-wav";

export class SessionWavWriter {
  readonly file: File;

  private handle: FileHandle | null;
  private dataBytes = 0;
  private sampleRate = 16_000;
  private channels = 1;
  private initialized = false;
  private finalized = false;
  private checkpointDataBytes = 0;

  constructor(sessionId: string) {
    const directory = new Directory(Paths.document, "sessions", sessionId);
    directory.create({ intermediates: true, idempotent: true });
    this.file = new File(directory, "audio.wav");
    this.file.create({ overwrite: true });
    this.handle = this.file.open(FileMode.ReadWrite);
  }

  append(buffer: ArrayBuffer, sampleRate: number, channels: number) {
    if (!this.handle || this.finalized) {
      throw new Error("Recording file is closed");
    }
    if (!this.initialized) {
      this.sampleRate = sampleRate;
      this.channels = channels;
      this.handle.writeBytes(wavHeader(0, sampleRate, channels));
      this.initialized = true;
    }
    if (sampleRate !== this.sampleRate || channels !== this.channels) {
      throw new Error("Audio stream format changed during recording");
    }
    const bytes = new Uint8Array(buffer);
    this.handle.writeBytes(bytes);
    this.dataBytes += bytes.byteLength;
    const checkpointIntervalBytes = this.sampleRate * this.channels * 2 * 5;
    if (this.dataBytes - this.checkpointDataBytes >= checkpointIntervalBytes) {
      this.writeHeader();
      this.checkpointDataBytes = this.dataBytes;
    }
  }

  finalize(): File {
    if (this.finalized) return this.file;
    if (!this.handle) throw new Error("Recording file is unavailable");
    if (!this.initialized) {
      this.handle.writeBytes(wavHeader(0, this.sampleRate, this.channels));
      this.initialized = true;
    }
    this.writeHeader();
    this.handle.close();
    this.handle = null;
    this.finalized = true;
    return this.file;
  }

  close() {
    if (this.handle && this.initialized) this.writeHeader();
    this.handle?.close();
    this.handle = null;
  }

  private writeHeader() {
    if (!this.handle) throw new Error("Recording file is unavailable");
    const header = wavHeader(this.dataBytes, this.sampleRate, this.channels);
    this.handle.offset = 0;
    this.handle.writeBytes(header);
    this.handle.offset = header.byteLength + this.dataBytes;
  }
}
