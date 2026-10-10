import { readBoundedTranscriptionResponse } from "./transcription-response.ts";

export async function readChatgptSummary(response: Response): Promise<string> {
  return parseChatgptSummary(
    await readBoundedTranscriptionResponse(response, 8 * 1024 * 1024),
  );
}

export function parseChatgptSummary(stream: string): string {
  let text = "";
  let completed = false;
  for (const frame of stream.split(/\r?\n\r?\n/)) {
    const data = frame
      .split(/\r?\n/)
      .filter((line) => line.startsWith("data:"))
      .map((line) => line.slice(5).trimStart())
      .join("\n");
    if (!data || data === "[DONE]") continue;
    let payload: unknown;
    try {
      payload = JSON.parse(data);
    } catch {
      throw new Error(
        "ChatGPT returned an invalid summary response. Try again.",
      );
    }
    if (!payload || typeof payload !== "object")
      throw new Error(
        "ChatGPT returned an invalid summary response. Try again.",
      );
    const event = payload as {
      type?: string;
      delta?: string;
      response?: {
        status?: string;
        output?: Array<{
          type?: string;
          content?: Array<{ type?: string; text?: string }>;
        }>;
      };
    };
    if (
      event.type === "error" ||
      event.type === "response.failed" ||
      event.type === "response.incomplete"
    )
      throw new Error("ChatGPT could not complete the summary. Try again.");
    if (
      event.type === "response.output_text.delta" &&
      typeof event.delta === "string"
    )
      text += event.delta;
    if (event.type === "response.completed") {
      if (event.response?.status !== "completed")
        throw new Error("ChatGPT could not complete the summary. Try again.");
      const final = event.response.output
        ?.filter((item) => item.type === "message")
        .flatMap((item) => item.content ?? [])
        .filter(
          (part) =>
            part.type === "output_text" && typeof part.text === "string",
        )
        .map((part) => part.text)
        .join("\n");
      text = final || text;
      completed = true;
    }
  }
  if (!completed)
    throw new Error("ChatGPT’s response was interrupted. Try again.");
  if (!text.trim()) throw new Error("ChatGPT returned an empty summary.");
  return text.trim();
}
