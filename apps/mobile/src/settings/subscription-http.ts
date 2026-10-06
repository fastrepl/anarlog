import { readBoundedTranscriptionResponse } from "../data/transcription-response.ts";

export function record(value: unknown): Record<string, unknown> {
  return value && typeof value === "object"
    ? (value as Record<string, unknown>)
    : {};
}

export async function subscriptionJson(
  url: string,
  init: RequestInit,
  fetcher: typeof fetch,
  timeout = 15_000,
  readErrors = false,
) {
  init.signal?.throwIfAborted();
  const controller = new AbortController();
  const abort = () => controller.abort();
  init.signal?.addEventListener("abort", abort, { once: true });
  const timer = setTimeout(abort, timeout);
  try {
    const response = await fetcher(url, {
      ...init,
      redirect: "error",
      signal: controller.signal,
    });
    if (!response.ok && !readErrors) {
      await response.body?.cancel();
      return { status: response.status, json: {} as Record<string, unknown> };
    }
    const body = await readBoundedTranscriptionResponse(
      response,
      init.method === "POST" || readErrors ? 128 * 1024 : 8 * 1024 * 1024,
    );
    controller.signal.throwIfAborted();
    try {
      return { status: response.status, json: record(JSON.parse(body)) };
    } catch {
      throw new Error(
        "The subscription provider returned an invalid response. Try again.",
      );
    }
  } finally {
    clearTimeout(timer);
    init.signal?.removeEventListener("abort", abort);
  }
}
