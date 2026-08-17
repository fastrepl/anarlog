const WAV_HEADER_BYTES = 44;

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

export function pcmAmplitude(buffer: ArrayBuffer): number {
  const samples = new Int16Array(buffer);
  let peak = 0;
  for (let index = 0; index < samples.length; index += 4) {
    peak = Math.max(peak, Math.abs(samples[index] ?? 0));
  }
  return Math.min(1, peak / 32_768);
}
