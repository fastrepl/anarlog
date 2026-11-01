import {
  Directory,
  File,
  FileMode,
  Paths,
  type FileHandle,
} from "expo-file-system";

import { recoverableWav, WAV_HEADER_BYTES, wavHeader } from "@/audio/pcm-wav";
import { id } from "@/lib/ids";

export class SessionWavWriter {
  readonly file: File;

  private handle: FileHandle | null;
  private dataBytes = 0;
  private sampleRate = 16_000;
  private channels = 1;
  private initialized = false;
  private finalized = false;
  private checkpointDataBytes = 0;
  readonly previousDurationMs: number;
  private initialDataBytes = 0;

  constructor(sessionId: string, previousFilename?: string) {
    const directory = new Directory(Paths.document, "sessions", sessionId);
    directory.create({ intermediates: true, idempotent: true });
    this.file = new File(
      directory,
      previousFilename ? `audio-resume-${id()}.wav` : "audio.wav",
    );
    if (previousFilename) {
      const previous = new File(directory, previousFilename);
      const reader = previous.open(FileMode.ReadOnly);
      let format;
      try {
        format = recoverableWav(
          reader.readBytes(WAV_HEADER_BYTES),
          previous.size,
        );
        if (
          !format ||
          format.sampleRate !== 16_000 ||
          format.channels !== 1 ||
          format.dataBytes + WAV_HEADER_BYTES !== previous.size
        ) {
          throw new Error(
            "Resume requires a 16 kHz mono WAV recording from mobile.",
          );
        }
      } finally {
        reader.close();
      }
      // Keep the cataloged file immutable while uploads or playback may read it.
      previous.copy(this.file);
      this.dataBytes =
        this.initialDataBytes =
        this.checkpointDataBytes =
          format.dataBytes;
      this.sampleRate = format.sampleRate;
      this.channels = format.channels;
      this.initialized = true;
    }
    if (this.file.exists && this.file.size <= WAV_HEADER_BYTES) {
      this.file.delete();
    }
    if (!previousFilename) this.file.create();
    this.handle = this.file.open(FileMode.ReadWrite);
    if (previousFilename)
      this.handle.offset = WAV_HEADER_BYTES + this.dataBytes;
    this.previousDurationMs = this.durationMs;
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

  get durationMs(): number {
    return (this.dataBytes * 1_000) / (this.sampleRate * this.channels * 2);
  }

  get previousSizeBytes(): number {
    return this.initialDataBytes > 0
      ? WAV_HEADER_BYTES + this.initialDataBytes
      : 0;
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

  closeAndDiscardEmpty() {
    this.close();
    if (this.dataBytes === this.initialDataBytes && this.file.exists)
      this.file.delete();
  }

  private writeHeader() {
    if (!this.handle) throw new Error("Recording file is unavailable");
    const header = wavHeader(this.dataBytes, this.sampleRate, this.channels);
    this.handle.offset = 0;
    this.handle.writeBytes(header);
    this.handle.offset = header.byteLength + this.dataBytes;
  }
}
