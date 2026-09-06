import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import ts from "typescript";

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
