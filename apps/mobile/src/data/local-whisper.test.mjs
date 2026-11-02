import assert from "node:assert/strict";
import { registerHooks } from "node:module";
import { test } from "node:test";

const files = new Map();
const fixture = (globalThis.localWhisperFixture = {
  downloadContent: "valid-model",
  failDownload: false,
  released: false,
});

const filesystemMock = `
  const files = globalThis.localWhisperFiles;
  function uriOf(...parts) {
    const [base, ...rest] = parts;
    const prefix = typeof base === "string" ? base : base.uri;
    const suffix = rest
      .flatMap((part) => (typeof part === "string" ? part : part.uri).split("/"))
      .filter(Boolean)
      .join("/");
    return suffix ? prefix + (prefix.endsWith("/") ? "" : "/") + suffix : prefix;
  }
  export const Paths = {document: "file:///documents"};
  export class Directory {
    constructor(...parts) { this.uri = uriOf(...parts); }
    create() {}
  }
  export class File {
    constructor(...parts) {
      this.uri = uriOf(...parts);
      this.name = this.uri.split("/").at(-1);
    }
    get exists() { return files.has(this.uri); }
    delete() { files.delete(this.uri); }
    async move(destination, options = {}) {
      if (destination.exists && !options.overwrite) throw new Error("exists");
      files.set(destination.uri, files.get(this.uri));
      files.delete(this.uri);
    }
    static createDownloadTask(_url, destination, options) {
      return {
        async downloadAsync() {
          options.onProgress({bytesWritten: 1, totalBytes: 1});
          files.set(destination.uri, globalThis.localWhisperFixture.downloadContent);
          if (globalThis.localWhisperFixture.failDownload) throw new Error("network failed");
          return destination;
        },
        release() { globalThis.localWhisperFixture.released = true; },
      };
    }
  }
`;

registerHooks({
  resolve(specifier, context, next) {
    if (specifier === "expo-file-system") {
      return {
        url: `data:text/javascript,${encodeURIComponent(filesystemMock)}`,
        shortCircuit: true,
      };
    }
    if (specifier === "@anlg/mobile-bridge") {
      const bridge = `
        export function localWhisperModelInfo() {
          return JSON.stringify({
            model: "Whisper Small Q8",
            url: "https://models.test/model.bin",
            uri: "file:///documents/models/stt/ggml-small-q8_0.bin",
            size_bytes: 12
          });
        }
        export async function verifyLocalWhisperModel(uri) {
          return globalThis.localWhisperFiles.get(uri) === "valid-model";
        }
        export async function transcribeLocalWhisper() { return "[]"; }
      `;
      return {
        url: `data:text/javascript,${encodeURIComponent(bridge)}`,
        shortCircuit: true,
      };
    }
    return next(specifier, context);
  },
});

globalThis.localWhisperFiles = files;
const { downloadLocalWhisperModel } = await import("./local-whisper.ts");

test("failed model download preserves the installed model and removes partial data", async () => {
  const modelUri = "file:///documents/models/stt/ggml-small-q8_0.bin";
  files.set(modelUri, "previous-valid-model");
  fixture.failDownload = true;
  fixture.downloadContent = "partial";
  fixture.released = false;

  await assert.rejects(
    downloadLocalWhisperModel(
      () => {},
      () => {},
    ),
    /network failed/,
  );

  assert.equal(files.get(modelUri), "previous-valid-model");
  assert.equal(
    [...files.keys()].some((uri) => uri.includes("/.downloads/")),
    false,
  );
  assert.equal(fixture.released, true);
});

test("verified model replaces the installed model only after download completes", async () => {
  const modelUri = "file:///documents/models/stt/ggml-small-q8_0.bin";
  files.set(modelUri, "previous-valid-model");
  fixture.failDownload = false;
  fixture.downloadContent = "valid-model";
  fixture.released = false;

  await downloadLocalWhisperModel(
    () => {},
    () => {},
  );

  assert.equal(files.get(modelUri), "valid-model");
  assert.equal(
    [...files.keys()].some((uri) => uri.includes("/.downloads/")),
    false,
  );
  assert.equal(fixture.released, true);
});
