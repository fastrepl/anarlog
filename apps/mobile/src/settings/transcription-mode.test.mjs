import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import ts from "typescript";

import { presetProviderModels } from "./provider-model-catalog.ts";
import { defaultProviderConfig, providersFor } from "./providers-model.ts";
import {
  batchTranscriptionModel,
  supportsLiveTranscription,
} from "./transcription-mode.ts";

test("mobile model modes match desktop for every preset remote model", () => {
  const source = readFileSync(
    new URL("../../../desktop/src/stt/capabilities.ts", import.meta.url),
    "utf8",
  );
  const functionText = source.slice(
    source.indexOf("export function getSttModelTranscriptionMode("),
    source.indexOf("function baseLanguageCode("),
  );
  const js = ts.transpile(functionText.replace("export function", "function"));
  const desktopMode = new Function(
    "isLocalFileSttModel",
    `${js}; return getSttModelTranscriptionMode;`,
  )(() => false);
  for (const { id } of providersFor("stt")) {
    for (const model of presetProviderModels("stt", id) ?? []) {
      assert.equal(
        supportsLiveTranscription(id, model),
        desktopMode(id, model) !== "batch",
        `${id}/${model}`,
      );
    }
  }
  assert.equal(supportsLiveTranscription("soniox", "stt-async-v5"), false);
  assert.equal(supportsLiveTranscription("custom", "whisper-1"), false);
  assert.equal(supportsLiveTranscription("assemblyai", ""), false);
});

test("a saved recording uses the corresponding batch model after live failure", () => {
  for (const [provider, live, batch] of [
    ["assemblyai", "universal-3-5-pro-realtime", "universal-3-5-pro"],
    ["openai", "gpt-live-transcribe", "gpt-transcribe"],
    ["deepgram", "flux-general-en", "nova-3-general"],
    ["soniox", "stt-rt-v5", "stt-async-v5"],
    ["soniox", "my-custom-model", "my-custom-model"],
    [
      "google_generative_ai",
      "gemini-3.5-transcribe-live",
      "gemini-3.5-transcribe",
    ],
    ["elevenlabs", "scribe_v2_realtime", "scribe_v2"],
    ["mistral", "voxtral-mini-transcribe-realtime-2602", "voxtral-mini-2602"],
    ["custom", "own-model", "own-model"],
    ["dashscope", "qwen3-asr-flash-realtime", null],
  ])
    assert.equal(batchTranscriptionModel(provider, live), batch);
});

test("new Soniox setups start live while saved batch models remain supported", () => {
  const config = defaultProviderConfig("stt", "soniox");
  assert.ok(presetProviderModels("stt", "soniox").includes(config.model));
  assert.equal(supportsLiveTranscription(config.provider, config.model), true);
  assert.equal(
    batchTranscriptionModel(config.provider, config.model),
    "stt-async-v5",
  );
  assert.equal(
    batchTranscriptionModel("soniox", "stt-async-v5"),
    "stt-async-v5",
  );
});
