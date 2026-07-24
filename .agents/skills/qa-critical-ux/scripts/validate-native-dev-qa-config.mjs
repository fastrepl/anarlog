#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

const expectedKeys = [
  "VITE_APP_URL",
  "VITE_API_URL",
  "VITE_SUPABASE_URL",
  "VITE_SUPABASE_ANON_KEY",
];

function fail(message) {
  console.error(message);
  process.exit(1);
}

function readSources(target) {
  if (target === "-") {
    return [{ path: "<stdin>", source: readFileSync(0, "utf8") }];
  }

  const metadata = statSync(target);
  if (metadata.isFile()) {
    return [{ path: target, source: readFileSync(target, "utf8") }];
  }
  if (!metadata.isDirectory()) {
    fail(`Compiled frontend target is not a file or directory: ${target}`);
  }

  const files = [];
  const visit = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) {
        visit(path);
      } else if (entry.isFile() && entry.name.endsWith(".js")) {
        files.push(path);
      }
    }
  };
  visit(target);
  files.sort();
  return files.map((path) => ({ path, source: readFileSync(path, "utf8") }));
}

function extractRuntimeEnvironments(sources) {
  const environments = [];
  const runtimeEnvironmentPattern =
    /runtimeEnv\s*:\s*\{([\s\S]*?)\}\s*,\s*emptyStringAsUndefined/g;
  const publicValuePattern =
    /["'](VITE_[A-Z0-9_]+)["']\s*:\s*["']([^"']*)["']/g;

  for (const { path, source } of sources) {
    for (const match of source.matchAll(runtimeEnvironmentPattern)) {
      const values = new Map();
      for (const valueMatch of match[1].matchAll(publicValuePattern)) {
        const [, key, value] = valueMatch;
        if (values.has(key) && values.get(key) !== value) {
          fail(`Compiled runtimeEnv defines ${key} more than once in ${path}.`);
        }
        values.set(key, value);
      }
      if (expectedKeys.some((key) => values.has(key))) {
        environments.push({ path, values });
      }
    }
  }

  return environments;
}

const target = process.argv[2];
if (!target || process.argv.length !== 3) {
  fail(`Usage: ${process.argv[1]} <compiled-frontend-directory|->`);
}

const expected = new Map();
for (const key of expectedKeys) {
  const value = process.env[`ANARLOG_QA_EXPECTED_${key}`];
  if (!value) {
    fail(`Missing expected public configuration: ${key}.`);
  }
  expected.set(key, value);
}

const environments = extractRuntimeEnvironments(readSources(target));
if (environments.length !== 1) {
  fail(
    `Expected one compiled runtimeEnv with native public configuration; found ${environments.length}.`,
  );
}

const [{ path, values }] = environments;
const actualKeys = [...values.keys()].sort();
const allowedKeys = [...expectedKeys].sort();
const unexpectedKeys = actualKeys.filter((key) => !expected.has(key));
const missingKeys = allowedKeys.filter((key) => !values.has(key));
if (unexpectedKeys.length > 0 || missingKeys.length > 0) {
  const details = [
    unexpectedKeys.length > 0
      ? `unexpected keys: ${unexpectedKeys.join(", ")}`
      : null,
    missingKeys.length > 0 ? `missing keys: ${missingKeys.join(", ")}` : null,
  ]
    .filter(Boolean)
    .join("; ");
  fail(`Compiled runtimeEnv is not allowlisted in ${path} (${details}).`);
}

for (const key of expectedKeys) {
  if (values.get(key) !== expected.get(key)) {
    fail(`Compiled runtimeEnv has the wrong value for ${key} in ${path}.`);
  }
}

const canonicalConfiguration = expectedKeys
  .map((key) => `${key}=${values.get(key)}\n`)
  .join("");
process.stdout.write(
  createHash("sha256").update(canonicalConfiguration).digest("hex"),
);
