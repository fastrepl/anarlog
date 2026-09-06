import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import ts from "typescript";

import {
  modelOptions,
  presetProviderModels,
} from "./provider-model-catalog.ts";
import { providersFor } from "./providers-model.ts";

function desktopProviders(kind) {
  const path = new URL(
    `../../../desktop/src/settings/ai/${kind}/shared.tsx`,
    import.meta.url,
  );
  const source = ts.createSourceFile(
    path.pathname,
    readFileSync(path, "utf8"),
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TSX,
  );
  let providers;
  const visit = (node) => {
    if (
      ts.isVariableDeclaration(node) &&
      node.name.getText(source) === "_PROVIDERS"
    ) {
      let initializer = node.initializer;
      while (
        ts.isSatisfiesExpression(initializer) ||
        ts.isAsExpression(initializer)
      )
        initializer = initializer.expression;
      providers = initializer.elements.map((element) =>
        Object.fromEntries(
          element.properties
            .filter(ts.isPropertyAssignment)
            .map((property) => [
              property.name.getText(source),
              property.initializer,
            ]),
        ),
      );
    }
    ts.forEachChild(node, visit);
  };
  visit(source);
  assert.ok(providers?.length);
  return providers.filter((provider) => {
    const baseUrl = ts.isStringLiteral(provider.baseUrl)
      ? provider.baseUrl.text
      : "";
    return (
      provider.id.text === "custom" ||
      (provider.requirements?.getText(source).includes('"api_key"') &&
        provider.authKind?.text !== "subscription" &&
        !baseUrl.startsWith("http://") &&
        provider.disabled?.kind !== ts.SyntaxKind.TrueKeyword)
    );
  });
}

test("transcription choices follow desktop's catalog without its live-only models", () => {
  const liveOnly = new Set([
    "flux-general-multi",
    "flux-general-en",
    "gpt-live-transcribe",
    "universal-3-5-pro-realtime",
    "gemini-3.5-transcribe-live",
    "scribe_v2_realtime",
    "voxtral-mini-transcribe-realtime-2602",
  ]);
  for (const provider of desktopProviders("stt")) {
    if (provider.id.text === "dashscope") continue;
    const expected = provider.models.elements
      .map((model) => model.text)
      .filter((model) => !liveOnly.has(model))
      .map((model) =>
        provider.id.text === "soniox"
          ? model.replace("stt-rt-", "stt-async-")
          : model,
      );
    assert.deepEqual(
      presetProviderModels("stt", provider.id.text),
      expected,
      provider.id.text,
    );
  }
});

test("model options retain a saved or manually entered ID without duplicates or empty options", () => {
  assert.deepEqual(modelOptions(["known", "known"], "custom-model"), [
    "known",
    "custom-model",
  ]);
  assert.deepEqual(modelOptions(["known"], "known"), ["known"]);
  assert.deepEqual(modelOptions([], ""), []);
  assert.deepEqual(modelOptions([], "manual"), ["manual"]);
});

for (const kind of ["stt", "llm"]) {
  test(`${kind} includes the desktop's remote API-key providers supported by mobile recording`, () => {
    const desktop = desktopProviders(kind).filter(
      (provider) => kind !== "stt" || provider.id.text !== "dashscope",
    );
    const mobile = providersFor(kind).filter(
      (provider) => provider.id !== "anarlog",
    );
    assert.deepEqual(
      mobile.map((provider) => provider.id).sort(),
      desktop.map((provider) => provider.id.text).sort(),
    );
    for (const provider of desktop) {
      const match = mobile.find((entry) => entry.id === provider.id.text);
      assert.equal(match.name, provider.displayName.text);
      if (ts.isStringLiteral(provider.baseUrl))
        assert.equal(match.baseUrl, provider.baseUrl.text);
    }
  });
}
