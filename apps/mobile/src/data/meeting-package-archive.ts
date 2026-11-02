import { Directory, File, FileMode, Paths } from "expo-file-system";
import { sha256 } from "js-sha256";

const CHUNK_BYTES = 64 * 1024;
const YIELD_AFTER_BYTES = 4 * 1024 * 1024;
const encoder = new TextEncoder();
const decoder = new TextDecoder();
const crcTable = Uint32Array.from({ length: 256 }, (_, index) => {
  let value = index;
  for (let bit = 0; bit < 8; bit++)
    value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
  return value >>> 0;
});

export type MeetingArchiveAudio = {
  contentType: string;
  filename: string;
  sha256: string;
  sizeBytes: number;
};

export async function writeMeetingArchive(
  destination: File,
  manifest: string,
  audio?: { file: File; descriptor: MeetingArchiveAudio },
): Promise<void> {
  const manifestBytes = encoder.encode(manifest);
  const output = openOutput(destination);
  const entries: CentralEntry[] = [];
  let offset = 0;
  try {
    const metadataName = "meeting.json";
    const metadataHeader = localHeader(
      metadataName,
      crc32(manifestBytes),
      manifestBytes.byteLength,
    );
    entries.push({
      name: metadataName,
      crc: crc32(manifestBytes),
      size: manifestBytes.byteLength,
      offset,
      flags: 0x0800,
    });
    writeBytes(output, metadataHeader);
    offset += metadataHeader.byteLength;
    writeBytes(output, manifestBytes);
    offset += manifestBytes.byteLength;

    if (audio) {
      const name = "audio/recording";
      const nameBytes = encoder.encode(name);
      const flags = 0x0808;
      const headerOffset = offset;
      const header = localHeader(name, 0, 0, flags);
      writeBytes(output, header);
      offset += header.byteLength;

      const input = audio.file.open(FileMode.ReadOnly);
      try {
        if (input.size !== audio.descriptor.sizeBytes || input.size === null) {
          throw new Error("The recording changed during package export.");
        }
        input.offset = 0;
        const digest = sha256.create();
        let remaining = audio.descriptor.sizeBytes;
        let bytesSinceYield = 0;
        let crc = 0xffffffff;
        while (remaining > 0) {
          const chunk = input.readBytes(Math.min(CHUNK_BYTES, remaining));
          if (chunk.byteLength === 0)
            throw new Error("The recording ended during package export.");
          digest.update(chunk);
          crc = updateCrc(crc, chunk);
          writeBytes(output, chunk);
          offset += chunk.byteLength;
          remaining -= chunk.byteLength;
          bytesSinceYield += chunk.byteLength;
          if (bytesSinceYield >= YIELD_AFTER_BYTES) {
            bytesSinceYield = 0;
            await yieldToUi();
          }
        }
        crc = (crc ^ 0xffffffff) >>> 0;
        if (digest.hex() !== audio.descriptor.sha256) {
          throw new Error("The recording changed during package export.");
        }
        const descriptor = dataBytes(16, (view) => {
          view.setUint32(0, 0x08074b50, true);
          view.setUint32(4, crc, true);
          view.setUint32(8, audio.descriptor.sizeBytes, true);
          view.setUint32(12, audio.descriptor.sizeBytes, true);
        });
        writeBytes(output, descriptor);
        offset += descriptor.byteLength;
        entries.push({
          name,
          crc,
          size: audio.descriptor.sizeBytes,
          offset: headerOffset,
          flags,
          descriptor: true,
        });
      } finally {
        input.close();
      }
      if (nameBytes.byteLength !== headerNameLength(header)) {
        throw new Error("Could not create the meeting archive.");
      }
    }

    const centralOffset = offset;
    for (const entry of entries) {
      const central = centralHeader(entry);
      writeBytes(output, central);
      offset += central.byteLength;
    }
    const centralSize = offset - centralOffset;
    const end = endRecord(entries.length, centralSize, centralOffset);
    writeBytes(output, end);
  } finally {
    output.close();
  }
}

export async function readMeetingArchive(
  source: File,
  limits: {
    maxArchiveBytes: number;
    maxManifestBytes: number;
    maxAudioBytes: number;
  },
): Promise<{
  manifest: string;
  audio: { file: File; sizeBytes: number; sha256: string } | null;
}> {
  if (source.size <= 0 || source.size > limits.maxArchiveBytes)
    throw invalidArchive();
  const input = source.open(FileMode.ReadOnly);
  const temporary = new File(
    new Directory(Paths.cache, "meeting-imports"),
    `${Date.now()}-${Math.random().toString(16).slice(2)}.audio`,
  );
  let temporaryCreated = false;
  try {
    if (input.size !== source.size || input.size === null)
      throw invalidArchive();
    const metadataHeader = readLocalHeader(input);
    if (
      metadataHeader.name !== "meeting.json" ||
      metadataHeader.method !== 0 ||
      metadataHeader.flags !== 0x0800 ||
      metadataHeader.extraLength !== 0 ||
      metadataHeader.compressedSize !== metadataHeader.size ||
      metadataHeader.size > limits.maxManifestBytes
    )
      throw invalidArchive();
    const manifestBytes = readBytes(input, metadataHeader.size);
    if (crc32(manifestBytes) !== metadataHeader.crc) throw invalidArchive();
    const manifest = decoder.decode(manifestBytes);
    const descriptor = parseAudioDescriptor(manifest);
    let audioResult: {
      file: File;
      sizeBytes: number;
      sha256: string;
    } | null = null;
    let audioCrc = 0;
    let audioLocalOffset = 0;

    if (descriptor) {
      if (descriptor.sizeBytes > limits.maxAudioBytes) throw invalidArchive();
      audioLocalOffset = input.offset ?? 0;
      const audioHeader = readLocalHeader(input);
      if (
        audioHeader.name !== "audio/recording" ||
        audioHeader.method !== 0 ||
        audioHeader.flags !== 0x0808 ||
        audioHeader.extraLength !== 0 ||
        audioHeader.size !== 0 ||
        audioHeader.compressedSize !== 0
      )
        throw invalidArchive();
      temporaryCreated = true;
      const output = openOutput(temporary);
      let crc = 0xffffffff;
      const digest = sha256.create();
      let remaining = descriptor.sizeBytes;
      let bytesSinceYield = 0;
      try {
        while (remaining > 0) {
          const chunk = input.readBytes(Math.min(CHUNK_BYTES, remaining));
          if (chunk.byteLength === 0) throw invalidArchive();
          crc = updateCrc(crc, chunk);
          digest.update(chunk);
          writeBytes(output, chunk);
          remaining -= chunk.byteLength;
          bytesSinceYield += chunk.byteLength;
          if (bytesSinceYield >= YIELD_AFTER_BYTES) {
            bytesSinceYield = 0;
            await yieldToUi();
          }
        }
      } finally {
        output.close();
      }
      crc = (crc ^ 0xffffffff) >>> 0;
      audioCrc = crc;
      const dataDescriptor = readBytes(input, 16);
      const descriptorView = new DataView(dataDescriptor.buffer);
      if (
        descriptorView.getUint32(0, true) !== 0x08074b50 ||
        descriptorView.getUint32(4, true) !== crc ||
        descriptorView.getUint32(8, true) !== descriptor.sizeBytes ||
        descriptorView.getUint32(12, true) !== descriptor.sizeBytes ||
        digest.hex() !== descriptor.sha256
      )
        throw invalidArchive();
      audioResult = {
        file: temporary,
        sizeBytes: descriptor.sizeBytes,
        sha256: descriptor.sha256,
      };
    }

    const count = descriptor ? 2 : 1;
    const centralOffset = input.offset ?? 0;
    for (let index = 0; index < count; index++) {
      const central = readBytes(input, 46);
      const view = new DataView(central.buffer);
      if (view.getUint32(0, true) !== 0x02014b50) throw invalidArchive();
      const nameLength = view.getUint16(28, true);
      const extraLength = view.getUint16(30, true);
      const commentLength = view.getUint16(32, true);
      const name = decoder.decode(readBytes(input, nameLength));
      const expected =
        index === 0
          ? {
              name: "meeting.json",
              flags: 0x0800,
              crc: metadataHeader.crc,
              size: metadataHeader.size,
              offset: 0,
            }
          : {
              name: "audio/recording",
              flags: 0x0808,
              crc: audioCrc,
              size: descriptor!.sizeBytes,
              offset: audioLocalOffset,
            };
      if (
        name !== expected.name ||
        view.getUint16(8, true) !== expected.flags ||
        view.getUint16(10, true) !== 0 ||
        view.getUint32(16, true) !== expected.crc ||
        view.getUint32(20, true) !== expected.size ||
        view.getUint32(24, true) !== expected.size ||
        view.getUint32(42, true) !== expected.offset ||
        extraLength !== 0 ||
        commentLength !== 0
      )
        throw invalidArchive();
      input.offset = (input.offset ?? 0) + extraLength + commentLength;
    }
    const centralSize = (input.offset ?? 0) - centralOffset;
    const end = readBytes(input, 22);
    const endView = new DataView(end.buffer);
    if (
      endView.getUint32(0, true) !== 0x06054b50 ||
      endView.getUint16(8, true) !== count ||
      endView.getUint16(10, true) !== count ||
      endView.getUint32(12, true) !== centralSize ||
      endView.getUint32(16, true) !== centralOffset ||
      endView.getUint16(20, true) !== 0 ||
      (input.offset ?? 0) !== source.size
    )
      throw invalidArchive();
    return { manifest, audio: audioResult };
  } catch (error) {
    if (temporaryCreated && temporary.exists) temporary.delete();
    if (
      error instanceof Error &&
      error.message !== "Invalid meeting package archive."
    )
      throw error;
    throw invalidArchive();
  } finally {
    input.close();
  }
}

function parseAudioDescriptor(raw: string): MeetingArchiveAudio | null {
  let payload: unknown;
  try {
    payload = JSON.parse(raw);
  } catch {
    throw invalidArchive();
  }
  if (!payload || typeof payload !== "object" || !("audio" in payload))
    throw invalidArchive();
  const audio = payload.audio;
  if (audio === null) return null;
  if (
    !audio ||
    typeof audio !== "object" ||
    !("content_type" in audio) ||
    typeof audio.content_type !== "string" ||
    !("filename" in audio) ||
    typeof audio.filename !== "string" ||
    !("sha256" in audio) ||
    typeof audio.sha256 !== "string" ||
    !/^[0-9a-f]{64}$/u.test(audio.sha256) ||
    !("size_bytes" in audio) ||
    typeof audio.size_bytes !== "number" ||
    !Number.isSafeInteger(audio.size_bytes) ||
    audio.size_bytes <= 0
  )
    throw invalidArchive();
  return {
    contentType: audio.content_type,
    filename: audio.filename,
    sha256: audio.sha256,
    sizeBytes: audio.size_bytes,
  };
}

type CentralEntry = {
  name: string;
  crc: number;
  size: number;
  offset: number;
  flags: number;
  descriptor?: boolean;
};

type LocalHeader = {
  name: string;
  flags: number;
  method: number;
  crc: number;
  compressedSize: number;
  size: number;
  extraLength: number;
};

function localHeader(
  name: string,
  crc: number,
  size: number,
  flags = 0x0800,
): Uint8Array {
  const nameBytes = encoder.encode(name);
  return dataBytes(30 + nameBytes.byteLength, (view) => {
    view.setUint32(0, 0x04034b50, true);
    view.setUint16(4, 20, true);
    view.setUint16(6, flags, true);
    view.setUint16(8, 0, true);
    view.setUint32(14, crc, true);
    view.setUint32(18, size, true);
    view.setUint32(22, size, true);
    view.setUint16(26, nameBytes.byteLength, true);
    view.setUint16(28, 0, true);
    new Uint8Array(view.buffer).set(nameBytes, 30);
  });
}

function centralHeader(entry: CentralEntry): Uint8Array {
  const nameBytes = encoder.encode(entry.name);
  return dataBytes(46 + nameBytes.byteLength, (view) => {
    view.setUint32(0, 0x02014b50, true);
    view.setUint16(4, 20, true);
    view.setUint16(6, 20, true);
    view.setUint16(8, entry.flags, true);
    view.setUint16(10, 0, true);
    view.setUint32(16, entry.crc, true);
    view.setUint32(20, entry.size, true);
    view.setUint32(24, entry.size, true);
    view.setUint16(28, nameBytes.byteLength, true);
    view.setUint16(30, 0, true);
    view.setUint16(32, 0, true);
    view.setUint16(34, 0, true);
    view.setUint16(36, 0, true);
    view.setUint32(38, 0, true);
    view.setUint32(42, entry.offset, true);
    new Uint8Array(view.buffer).set(nameBytes, 46);
  });
}

function endRecord(count: number, centralSize: number, centralOffset: number) {
  return dataBytes(22, (view) => {
    view.setUint32(0, 0x06054b50, true);
    view.setUint16(4, 0, true);
    view.setUint16(6, 0, true);
    view.setUint16(8, count, true);
    view.setUint16(10, count, true);
    view.setUint32(12, centralSize, true);
    view.setUint32(16, centralOffset, true);
    view.setUint16(20, 0, true);
  });
}

function readLocalHeader(input: ReturnType<File["open"]>): LocalHeader {
  const bytes = readBytes(input, 30);
  const view = new DataView(bytes.buffer);
  if (view.getUint32(0, true) !== 0x04034b50) throw invalidArchive();
  const nameLength = view.getUint16(26, true);
  const extraLength = view.getUint16(28, true);
  const name = decoder.decode(readBytes(input, nameLength));
  input.offset = (input.offset ?? 0) + extraLength;
  return {
    name,
    flags: view.getUint16(6, true),
    method: view.getUint16(8, true),
    crc: view.getUint32(14, true),
    compressedSize: view.getUint32(18, true),
    size: view.getUint32(22, true),
    extraLength,
  };
}

function readBytes(input: ReturnType<File["open"]>, size: number): Uint8Array {
  const bytes = input.readBytes(size);
  if (bytes.byteLength !== size) throw invalidArchive();
  return bytes;
}

function openOutput(file: File) {
  if (file.exists) file.delete();
  file.create({ intermediates: true, overwrite: true });
  return file.open(FileMode.WriteOnly);
}

function writeBytes(handle: ReturnType<File["open"]>, bytes: Uint8Array): void {
  handle.writeBytes(bytes);
}

function dataBytes(
  length: number,
  write: (view: DataView) => void,
): Uint8Array {
  const bytes = new Uint8Array(length);
  write(new DataView(bytes.buffer));
  return bytes;
}

function headerNameLength(header: Uint8Array): number {
  return new DataView(header.buffer).getUint16(26, true);
}

function crc32(bytes: Uint8Array): number {
  return (updateCrc(0xffffffff, bytes) ^ 0xffffffff) >>> 0;
}

function updateCrc(crc: number, bytes: Uint8Array): number {
  let value = crc;
  for (const byte of bytes)
    value = crcTable[(value ^ byte) & 0xff]! ^ (value >>> 8);
  return value >>> 0;
}

function yieldToUi(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

function invalidArchive(): Error {
  return new Error("Invalid meeting package archive.");
}
