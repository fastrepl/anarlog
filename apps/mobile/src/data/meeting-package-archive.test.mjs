import { sha256 } from "js-sha256";
import assert from "node:assert/strict";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  openSync,
  readFileSync,
  readSync,
  statSync,
  unlinkSync,
  writeFileSync,
  writeSync,
  closeSync,
} from "node:fs";
import { registerHooks } from "node:module";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { test } from "node:test";

const root = mkdtempSync(join(tmpdir(), "anarlog-meeting-archive-"));
globalThis.meetingArchiveFixture = { root };
const fileSystemMock = `
import { closeSync, copyFileSync, existsSync, mkdirSync, openSync, readSync, statSync, unlinkSync, writeFileSync, writeSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
const root = globalThis.meetingArchiveFixture.root;
function pathOf(value) {
  if (value && typeof value === 'object' && 'path' in value) return value.path;
  const text = String(value);
  return text.startsWith('file://') ? text.slice(7) : text;
}
export const Paths = { cache: join(root, 'cache'), document: join(root, 'document') };
export const FileMode = { ReadOnly: 'r', WriteOnly: 'w' };
export class Directory {
  constructor(...parts) { this.path = resolve(join(...parts.map(pathOf))); }
  create({ intermediates = false } = {}) {
    mkdirSync(this.path, { recursive: intermediates });
  }
}
export class File {
  constructor(...parts) { this.path = resolve(join(...parts.map(pathOf))); }
  get uri() { return this.path; }
  get size() { return existsSync(this.path) ? statSync(this.path).size : 0; }
  get exists() { return existsSync(this.path); }
  create({ intermediates = false, overwrite = false } = {}) {
    if (intermediates) mkdirSync(dirname(this.path), { recursive: true });
    if (existsSync(this.path) && !overwrite) throw new Error('exists');
    writeFileSync(this.path, Buffer.alloc(0));
  }
  write(value) { mkdirSync(dirname(this.path), { recursive: true }); writeFileSync(this.path, value); }
  delete() { if (existsSync(this.path)) unlinkSync(this.path); }
  async copy(destination) { mkdirSync(dirname(destination.path), { recursive: true }); copyFileSync(this.path, destination.path); }
  open(mode) {
    const fd = openSync(this.path, mode);
    const filePath = this.path;
    let cursor = 0;
    return {
      get offset() { return cursor; }, set offset(value) { cursor = value; },
      get size() { return statSync(filePath).size; },
      readBytes(length) { const bytes = Buffer.alloc(length); const count = readSync(fd, bytes, 0, length, cursor); cursor += count; return new Uint8Array(bytes.buffer, bytes.byteOffset, count); },
      writeBytes(bytes) { cursor += writeSync(fd, bytes, 0, bytes.byteLength, cursor); },
      close() { closeSync(fd); },
    };
  }
}
`;
registerHooks({
  resolve(specifier, context, next) {
    if (specifier === "expo-file-system") {
      return {
        url: `data:text/javascript,${encodeURIComponent(fileSystemMock)}`,
        shortCircuit: true,
      };
    }
    return next(specifier, context);
  },
});

const { File, Paths } = await import("expo-file-system");
const { readMeetingArchive, writeMeetingArchive } =
  await import("./meeting-package-archive.ts");

function makeAudio(size) {
  const bytes = Uint8Array.from(
    { length: size },
    (_, index) => (index * 31) & 0xff,
  );
  const file = new File(join(root, `source-${size}.audio`));
  file.write(bytes);
  return {
    bytes,
    file,
    descriptor: {
      contentType: "audio/wav",
      filename: "recording.wav",
      sha256: sha256(bytes),
      sizeBytes: size,
    },
  };
}

function limits() {
  return {
    maxArchiveBytes: 2_000_000,
    maxManifestBytes: 100_000,
    maxAudioBytes: 1_000_000,
  };
}

test("streams audio and manifest through an Anarlog ZIP package round trip", async () => {
  const audio = makeAudio(700_001);
  const manifest = JSON.stringify({
    format: "anarlog.meeting",
    version: 1,
    audio: {
      content_type: audio.descriptor.contentType,
      filename: audio.descriptor.filename,
      sha256: audio.descriptor.sha256,
      size_bytes: audio.descriptor.sizeBytes,
    },
  });
  const archiveFile = new File(Paths.cache, "round-trip.anlgmeeting");
  await writeMeetingArchive(archiveFile, manifest, {
    file: audio.file,
    descriptor: audio.descriptor,
  });
  const archive = await readMeetingArchive(archiveFile, limits());
  assert.equal(archive.manifest, manifest);
  assert.ok(archive.audio);
  assert.equal(archive.audio.sizeBytes, audio.bytes.byteLength);
  assert.equal(archive.audio.sha256, audio.descriptor.sha256);
  assert.deepEqual(
    readFileSync(archive.audio.file.path),
    Buffer.from(audio.bytes),
  );
  archive.audio.file.delete();
});

test("reads text-only packages and rejects modified audio payloads", async () => {
  const noAudio = new File(Paths.cache, "text-only.anlgmeeting");
  const manifest = JSON.stringify({
    format: "anarlog.meeting",
    version: 1,
    audio: null,
  });
  await writeMeetingArchive(noAudio, manifest);
  const textArchive = await readMeetingArchive(noAudio, limits());
  assert.equal(textArchive.manifest, manifest);
  assert.equal(textArchive.audio, null);

  const audio = makeAudio(64_000);
  const audioManifest = JSON.stringify({
    format: "anarlog.meeting",
    version: 1,
    audio: {
      content_type: audio.descriptor.contentType,
      filename: audio.descriptor.filename,
      sha256: audio.descriptor.sha256,
      size_bytes: audio.descriptor.sizeBytes,
    },
  });
  const damaged = new File(Paths.cache, "damaged.anlgmeeting");
  await writeMeetingArchive(damaged, audioManifest, {
    file: audio.file,
    descriptor: audio.descriptor,
  });
  const archiveBytes = readFileSync(damaged.path);
  const marker = Buffer.from("audio/recording");
  const markerOffset = archiveBytes.indexOf(marker);
  // The local entry header precedes its name; skip that entry and flip an audio byte.
  const localHeaderOffset = archiveBytes.lastIndexOf(
    Buffer.from([0x50, 0x4b, 0x03, 0x04]),
    markerOffset,
  );
  const dataOffset = localHeaderOffset + 30 + marker.length;
  archiveBytes[dataOffset + 8] ^= 0xff;
  writeFileSync(damaged.path, archiveBytes);
  await assert.rejects(
    readMeetingArchive(damaged, limits()),
    /Invalid meeting package archive/u,
  );
});
