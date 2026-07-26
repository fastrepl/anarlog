import { describe, expect, it } from "vitest";

import {
  extractMcpOutputText,
  parseMcpObjectOutput,
  parseMcpToolOutput,
  readMcpJsonText,
} from "./mcp-output-parser";

describe("mcp-output-parser", () => {
  it("extracts text content from MCP output", () => {
    expect(
      extractMcpOutputText({
        content: [
          { type: "text", text: '{"status":"ok"}' },
          { type: "image", text: "ignored" },
        ],
      }),
    ).toBe('{"status":"ok"}');
  });

  it("parses JSON text payloads", () => {
    expect(
      readMcpJsonText({
        content: [{ type: "text", text: '{"status":"ok"}' }],
      }),
    ).toEqual({ status: "ok" });
  });

  it("parses object-shaped MCP outputs generically", () => {
    expect(
      parseMcpObjectOutput<{ status: string }>({
        content: [{ type: "text", text: '{"status":"applied"}' }],
      }),
    ).toEqual({ status: "applied" });
  });

  it("preserves plain object tool outputs", () => {
    expect(
      parseMcpObjectOutput<{ status: string; message: string }>({
        status: "error",
        message: "No active session selected.",
      }),
    ).toEqual({
      status: "error",
      message: "No active session selected.",
    });
  });

  it("parses guarded MCP outputs", () => {
    expect(
      parseMcpToolOutput<{ count: number }>(
        {
          content: [{ type: "text", text: '{"count":3}' }],
        },
        (value): value is { count: number } =>
          !!value &&
          typeof value === "object" &&
          "count" in value &&
          typeof value.count === "number",
      ),
    ).toEqual({ count: 3 });
  });
});
