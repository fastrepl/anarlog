import { describe, expect, it } from "vitest";

import {
  extractMcpOutputText,
  parseMcpObjectOutput,
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
});
