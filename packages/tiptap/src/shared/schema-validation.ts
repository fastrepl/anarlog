import { type Extensions, getSchema } from "@tiptap/core";
import type { Schema } from "@tiptap/pm/model";
import type { JSONContent } from "@tiptap/react";

import { getExtensions } from "./extensions";

let _schema: Schema | null = null;

function getCachedSchema(): Schema {
  if (!_schema) {
    _schema = getSchema(getExtensions() as Extensions);
  }
  return _schema;
}

export type SchemaValidationResult =
  | { valid: true }
  | { valid: false; error: string };

export function validateJsonContent(json: JSONContent): SchemaValidationResult {
  try {
    getCachedSchema().nodeFromJSON(json);
    return { valid: true };
  } catch (error) {
    return {
      valid: false,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

export function assertValidSchema(json: JSONContent): void {
  const result = validateJsonContent(json);
  if (!result.valid) {
    throw new Error(`Schema validation failed: ${result.error}`);
  }
}
