import { fetch } from "@tauri-apps/plugin-http";

import type { DeliveryItem, DeliveryPage, ScheduledCapture } from "./types";

const REQUEST_TIMEOUT_MS = 30_000;
const MAX_PAGE_BYTES = 24 * 1024 * 1024;

export class EnterpriseCaptureClientError extends Error {
  constructor(
    public readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "EnterpriseCaptureClientError";
  }
}

export async function listSessionDeliveries(input: {
  serverUrl: string;
  accessToken: string;
  workspaceId: string;
  consumerId: string;
  after: number;
  limit?: number;
}): Promise<DeliveryPage> {
  const url = endpoint(
    input.serverUrl,
    `v1/workspaces/${encodeURIComponent(input.workspaceId)}/session-envelopes`,
  );
  url.searchParams.set("consumerId", input.consumerId);
  url.searchParams.set("after", String(input.after));
  url.searchParams.set("limit", String(input.limit ?? 10));
  return parseDeliveryPage(await request(url, input.accessToken));
}

export async function acknowledgeSessionDelivery(input: {
  serverUrl: string;
  accessToken: string;
  workspaceId: string;
  consumerId: string;
  jobId: string;
  revision: number;
  contentHash: string;
}): Promise<void> {
  const body = await request(
    endpoint(
      input.serverUrl,
      `v1/workspaces/${encodeURIComponent(input.workspaceId)}/session-envelopes/${encodeURIComponent(input.jobId)}/ack`,
    ),
    input.accessToken,
    {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        consumerId: input.consumerId,
        revision: input.revision,
        contentHash: input.contentHash,
      }),
    },
  );
  if (!isObject(body) || body.acknowledged !== true) {
    throw new EnterpriseCaptureClientError(
      "invalid_response",
      "The capture server returned an invalid acknowledgement.",
    );
  }
}

export async function listScheduledCaptures(input: {
  serverUrl: string;
  accessToken: string;
  workspaceId: string;
}): Promise<ScheduledCapture[]> {
  const body = await request(
    endpoint(
      input.serverUrl,
      `v1/workspaces/${encodeURIComponent(input.workspaceId)}/scheduled-captures`,
    ),
    input.accessToken,
  );
  if (!Array.isArray(body)) {
    throw new EnterpriseCaptureClientError(
      "invalid_response",
      "The capture server returned an invalid scheduled capture list.",
    );
  }
  return body.map(parseScheduledCapture);
}

export async function cancelScheduledCapture(input: {
  serverUrl: string;
  accessToken: string;
  workspaceId: string;
  calendarEventId: string;
}): Promise<ScheduledCapture> {
  return parseScheduledCapture(
    await request(
      endpoint(
        input.serverUrl,
        `v1/workspaces/${encodeURIComponent(input.workspaceId)}/scheduled-captures/${encodeURIComponent(input.calendarEventId)}`,
      ),
      input.accessToken,
      { method: "DELETE" },
    ),
  );
}

async function request(
  url: URL,
  accessToken: string,
  init: RequestInit = {},
): Promise<unknown> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);
  try {
    const headers = new Headers(init.headers);
    headers.set("Authorization", `Bearer ${accessToken}`);
    const response = await fetch(url.toString(), {
      ...init,
      headers,
      signal: controller.signal,
    });
    const body = await boundedJson(response).catch((error) => {
      if (
        !response.ok &&
        error instanceof EnterpriseCaptureClientError &&
        error.code === "invalid_response"
      ) {
        return null;
      }
      throw error;
    });
    if (!response.ok) {
      const serverError =
        isObject(body) && isObject(body.error) ? body.error : null;
      throw new EnterpriseCaptureClientError(
        typeof serverError?.code === "string"
          ? serverError.code
          : `http_${response.status}`,
        typeof serverError?.message === "string"
          ? serverError.message
          : `Capture server request failed (${response.status}).`,
      );
    }
    return body;
  } catch (error) {
    if (error instanceof EnterpriseCaptureClientError) throw error;
    if (controller.signal.aborted) {
      throw new EnterpriseCaptureClientError(
        "request_timeout",
        "The capture server request timed out.",
      );
    }
    throw error;
  } finally {
    clearTimeout(timeout);
  }
}

async function boundedJson(response: Response): Promise<unknown> {
  const body = await boundedText(response);
  try {
    return JSON.parse(body) as unknown;
  } catch {
    throw new EnterpriseCaptureClientError(
      "invalid_response",
      "The capture server returned invalid JSON.",
    );
  }
}

async function boundedText(response: Response): Promise<string> {
  const declaredLength = Number(response.headers.get("content-length"));
  if (declaredLength > MAX_PAGE_BYTES) responseTooLarge();
  if (!response.body) return "";

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let body = "";
  let bytesRead = 0;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      bytesRead += value.byteLength;
      if (bytesRead > MAX_PAGE_BYTES) {
        await reader.cancel();
        responseTooLarge();
      }
      body += decoder.decode(value, { stream: true });
    }
    return body + decoder.decode();
  } finally {
    reader.releaseLock();
  }
}

function responseTooLarge(): never {
  throw new EnterpriseCaptureClientError(
    "response_too_large",
    "The capture server response is too large.",
  );
}

function parseDeliveryPage(value: unknown): DeliveryPage {
  if (!isObject(value) || !Array.isArray(value.items)) invalidResponse();
  const items = value.items.map(parseDeliveryItem);
  const nextCursor = integer(value.nextCursor, "next cursor", true);
  if (typeof value.hasMore !== "boolean") invalidResponse();
  return { items, nextCursor, hasMore: value.hasMore };
}

function parseDeliveryItem(value: unknown): DeliveryItem {
  if (!isObject(value) || !isObject(value.envelope)) invalidResponse();
  const envelope = value.envelope;
  if (
    integer(envelope.schema_version, "schema version") < 1 ||
    typeof envelope.source_id !== "string" ||
    envelope.source_id.length === 0 ||
    typeof envelope.finalized !== "boolean" ||
    typeof envelope.workspace_id !== "string" ||
    !isObject(envelope.session) ||
    typeof envelope.session.id !== "string" ||
    typeof envelope.session.status !== "string"
  ) {
    invalidResponse();
  }
  const revision = integer(value.revision, "revision");
  if (integer(envelope.revision, "envelope revision") !== revision) {
    invalidResponse();
  }
  if (
    typeof value.jobId !== "string" ||
    value.jobId.length === 0 ||
    typeof value.contentHash !== "string" ||
    !/^[0-9a-f]{64}$/.test(value.contentHash) ||
    typeof value.acknowledged !== "boolean" ||
    typeof value.finalized !== "boolean" ||
    value.finalized !== envelope.finalized ||
    typeof value.createdAt !== "string" ||
    !Number.isFinite(Date.parse(value.createdAt))
  ) {
    invalidResponse();
  }
  return {
    cursor: integer(value.cursor, "cursor"),
    jobId: value.jobId,
    revision,
    finalized: value.finalized,
    contentHash: value.contentHash,
    acknowledged: value.acknowledged,
    createdAt: value.createdAt,
    envelope: envelope as DeliveryItem["envelope"],
  };
}

function parseScheduledCapture(value: unknown): ScheduledCapture {
  if (
    !isObject(value) ||
    typeof value.calendarEventId !== "string" ||
    value.calendarEventId.length === 0 ||
    typeof value.title !== "string" ||
    typeof value.startsAt !== "string" ||
    !Number.isFinite(Date.parse(value.startsAt)) ||
    (value.status !== "pending" &&
      value.status !== "skipped" &&
      value.status !== "canceled" &&
      value.status !== "dispatched") ||
    (value.jobId !== null && typeof value.jobId !== "string")
  ) {
    throw new EnterpriseCaptureClientError(
      "invalid_response",
      "The capture server returned an invalid scheduled capture.",
    );
  }
  return {
    calendarEventId: value.calendarEventId,
    title: value.title,
    startsAt: value.startsAt,
    status: value.status,
    jobId: value.jobId,
  };
}

function integer(value: unknown, _field: string, allowZero = false): number {
  if (
    typeof value !== "number" ||
    !Number.isSafeInteger(value) ||
    (allowZero ? value < 0 : value < 1)
  ) {
    invalidResponse();
  }
  return value;
}

function invalidResponse(): never {
  throw new EnterpriseCaptureClientError(
    "invalid_response",
    "The capture server returned an invalid delivery envelope.",
  );
}

function endpoint(serverUrl: string, path: string): URL {
  const base = new URL(serverUrl);
  if (!base.pathname.endsWith("/")) base.pathname += "/";
  return new URL(path, base);
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
