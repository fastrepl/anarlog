import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { runInNewContext } from "node:vm";

const source = readFileSync(
  new URL("../../../desktop/src/settings/schema.ts", import.meta.url),
  "utf8",
);
const match = source.match(
  /export const SETTING_DEFINITIONS = (\{[\s\S]*?\}) as const;/,
);
assert(match, "Canonical settings declaration not found");
const definitions = runInNewContext(
  `(${match[1].replace(/ as (?:boolean|number|string)\b/g, "")})`,
  Object.create(null),
  { timeout: 1000 },
);
for (const [key, definition] of Object.entries(definitions)) {
  assert.match(key, /^[a-z_][a-z_0-9]*$/);
  assert(["boolean", "number", "string"].includes(definition.type));
  assert.equal(definition.path.length, 2);
  assert(definition.path.every((part) => typeof part === "string"));
  if ("default" in definition) {
    assert.equal(typeof definition.default, definition.type);
  }
}
const output = `${JSON.stringify(definitions, null, 2)}\n`;
const target = new URL("./settings-schema.json", import.meta.url);
if (process.argv.includes("--check")) {
  assert.equal(
    readFileSync(target, "utf8"),
    output,
    "Regenerate settings-schema.json",
  );
} else {
  writeFileSync(fileURLToPath(target), output);
}
