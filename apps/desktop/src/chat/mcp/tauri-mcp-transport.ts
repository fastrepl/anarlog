import type { JSONRPCMessage, MCPTransport } from "@ai-sdk/mcp";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";

const PROTOCOL_VERSION = "2025-06-18";

export class TauriMCPTransport implements MCPTransport {
  private url: string;
  private headers: Record<string, string>;
  private sessionId?: string;
  private abortController?: AbortController;
  private sseReader?: ReadableStreamDefaultReader<Uint8Array>;

  onclose?: () => void;
  onerror?: (error: Error) => void;
  onmessage?: (message: JSONRPCMessage) => void;

  constructor(url: string, headers: Record<string, string> = {}) {
    this.url = url;
    this.headers = headers;
  }

  private buildHeaders(
    extra: Record<string, string> = {},
  ): Record<string, string> {
    return {
      ...this.headers,
      ...extra,
      "mcp-protocol-version": PROTOCOL_VERSION,
      ...(this.sessionId && { "mcp-session-id": this.sessionId }),
    };
  }

  async start(): Promise<void> {
    this.abortController = new AbortController();
  }

  async send(message: JSONRPCMessage): Promise<void> {
    try {
      const response = await tauriFetch(this.url, {
        method: "POST",
        headers: this.buildHeaders({
          "Content-Type": "application/json",
          Accept: "application/json, text/event-stream",
        }),
        body: JSON.stringify(message),
        signal: this.abortController?.signal,
      });

      const sid = response.headers.get("mcp-session-id");
      if (sid) {
        const isFirstSession = !this.sessionId;
        this.sessionId = sid;
        if (isFirstSession) {
          this.openInboundSse();
        }
      }

      if (response.status === 202) return;
      if (!response.ok) {
        throw new Error(
          `HTTP ${response.status}: ${await response.text().catch(() => "")}`,
        );
      }
      if (!("id" in message)) return;

      const ct = response.headers.get("content-type") ?? "";
      const body = await response.text();

      const parsed = ct.includes("text/event-stream")
        ? parseSseText(body)
        : [JSON.parse(body)].flat();

      for (const m of parsed) {
        this.onmessage?.(m as JSONRPCMessage);
      }
    } catch (error) {
      if (error instanceof Error && error.name === "AbortError") return;
      this.onerror?.(error instanceof Error ? error : new Error(String(error)));
      throw error;
    }
  }

  private openInboundSse(): void {
    tauriFetch(this.url, {
      method: "GET",
      headers: this.buildHeaders({ Accept: "text/event-stream" }),
      signal: this.abortController?.signal,
    })
      .then((response) => {
        if (!response.ok || !response.body) return;

        const sid = response.headers.get("mcp-session-id");
        if (sid) this.sessionId = sid;

        this.sseReader = response.body.getReader();
        const decoder = new TextDecoder();
        let buffer = "";

        const pump = async () => {
          try {
            while (true) {
              const { done, value } = await this.sseReader!.read();
              if (done) break;
              buffer += decoder.decode(value, { stream: true });

              const parts = buffer.split("\n\n");
              buffer = parts.pop() ?? "";

              for (const part of parts) {
                for (const msg of parseSseBlock(part)) {
                  this.onmessage?.(msg);
                }
              }
            }
          } catch (error) {
            if (error instanceof Error && error.name === "AbortError") return;
          }
        };

        void pump();
      })
      .catch(() => {});
  }

  async close(): Promise<void> {
    this.sseReader?.cancel().catch(() => {});
    if (
      this.sessionId &&
      this.abortController &&
      !this.abortController.signal.aborted
    ) {
      await tauriFetch(this.url, {
        method: "DELETE",
        headers: this.buildHeaders(),
      }).catch(() => {});
    }
    this.abortController?.abort();
    this.onclose?.();
  }
}

function parseSseBlock(block: string): JSONRPCMessage[] {
  const trimmed = block.trim();
  if (!trimmed) return [];

  let eventType: string | undefined;
  const dataLines: string[] = [];

  for (const line of trimmed.split("\n")) {
    if (line.startsWith("event:")) {
      eventType = line.slice(6).trim();
    } else if (line.startsWith("data:")) {
      dataLines.push(line.slice(5).trimStart());
    }
  }

  if (eventType && eventType !== "message") return [];

  const data = dataLines.join("\n");
  if (!data) return [];

  try {
    return [JSON.parse(data) as JSONRPCMessage];
  } catch {
    return [];
  }
}

function parseSseText(text: string): JSONRPCMessage[] {
  return text.split(/\n\n+/).flatMap(parseSseBlock);
}
