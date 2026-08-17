export const WAV_HEADER_BYTES = 44;

export type RecoverableWav = {
  channels: number;
  dataBytes: number;
  sampleRate: number;
};

function writeAscii(view: DataView, offset: number, value: string) {
  for (let index = 0; index < value.length; index += 1) {
    view.setUint8(offset + index, value.charCodeAt(index));
  }
}

export function wavHeader(
  dataBytes: number,
  sampleRate: number,
  channels: number,
): Uint8Array {
  const buffer = new ArrayBuffer(WAV_HEADER_BYTES);
  const view = new DataView(buffer);
  const bytesPerSample = 2;
  writeAscii(view, 0, "RIFF");
  view.setUint32(4, 36 + dataBytes, true);
  writeAscii(view, 8, "WAVE");
  writeAscii(view, 12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, channels, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * channels * bytesPerSample, true);
  view.setUint16(32, channels * bytesPerSample, true);
  view.setUint16(34, bytesPerSample * 8, true);
  writeAscii(view, 36, "data");
  view.setUint32(40, dataBytes, true);
  return new Uint8Array(buffer);
}

function ascii(bytes: Uint8Array, offset: number, length: number): string {
  let value = "";
  for (let index = 0; index < length; index += 1) {
    value += String.fromCharCode(bytes[offset + index] ?? 0);
  }
  return value;
}

export function recoverableWav(
  header: Uint8Array,
  fileSize: number,
): RecoverableWav | null {
  if (
    header.byteLength < WAV_HEADER_BYTES ||
    !Number.isSafeInteger(fileSize) ||
    fileSize <= WAV_HEADER_BYTES
  ) {
    return null;
  }
  if (
    ascii(header, 0, 4) !== "RIFF" ||
    ascii(header, 8, 4) !== "WAVE" ||
    ascii(header, 12, 4) !== "fmt " ||
    ascii(header, 36, 4) !== "data"
  ) {
    return null;
  }

  const view = new DataView(
    header.buffer,
    header.byteOffset,
    header.byteLength,
  );
  const channels = view.getUint16(22, true);
  const sampleRate = view.getUint32(24, true);
  const blockAlign = view.getUint16(32, true);
  if (
    view.getUint32(16, true) !== 16 ||
    view.getUint16(20, true) !== 1 ||
    view.getUint16(34, true) !== 16 ||
    channels < 1 ||
    channels > 2 ||
    sampleRate < 8_000 ||
    sampleRate > 192_000 ||
    blockAlign !== channels * 2 ||
    view.getUint32(28, true) !== sampleRate * blockAlign
  ) {
    return null;
  }

  const dataBytes =
    Math.floor((fileSize - WAV_HEADER_BYTES) / blockAlign) * blockAlign;
  if (dataBytes <= 0 || dataBytes > 0xffff_ffff - 36) return null;
  return { channels, dataBytes, sampleRate };
}

export function pcmAmplitude(buffer: ArrayBuffer): number {
  const samples = new Int16Array(buffer);
  let peak = 0;
  for (let index = 0; index < samples.length; index += 4) {
    peak = Math.max(peak, Math.abs(samples[index] ?? 0));
  }
  return Math.min(1, peak / 32_768);
}
