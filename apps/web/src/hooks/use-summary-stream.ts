import { useCallback, useRef, useState } from "react";

import type { Segment } from "@hypr/transcript";

import { env } from "@/env";
import { getAccessToken } from "@/functions/access-token";

const SYSTEM_PROMPT = `You are an expert at creating structured, comprehensive meeting summaries.

# Format Requirements

- Use Markdown format without code block wrappers.
- Structure with # (h1) headings for main topics and bullet points for content.
- Use only h1 headers. Do not use h2 or h3. Each header represents a section.
- Each section should have at least 3 detailed bullet points.
- Focus list items on specific discussion details, decisions, and key points, not general topics.
- Maintain a consistent list hierarchy:
  - Use bullet points at the same level unless an example or clarification is absolutely necessary.
  - Avoid nesting lists beyond one level of indentation.
- Your final output MUST be ONLY the markdown summary itself.
- Do not include any explanations, commentary, or meta-discussion.
- Do not say things like "Here's the summary" or "I've analyzed".

# Guidelines

- Preserve essential details; avoid excessive abstraction. Ensure content remains concrete and specific.
- Do not include meeting note title, attendee lists nor explanatory notes about the output structure.`;

function segmentsToText(segments: Segment[]): string {
  return segments
    .map((seg) => {
      const label =
        seg.key.speaker_human_id ??
        (seg.key.speaker_index != null
          ? `Speaker ${seg.key.speaker_index}`
          : "Unknown");
      const text = seg.words
        .map((w) => w.text)
        .join("")
        .trim();
      return `${label}: ${text}`;
    })
    .join("\n");
}

function buildUserPrompt(segments: Segment[]): string {
  return `# Transcript

${segmentsToText(segments)}

# Instructions

1. Analyze the content and identify the main topics discussed.
2. Generate a well-formatted markdown summary with h1 section headings and bullet points.`;
}

export function useSummaryStream() {
  const [summary, setSummary] = useState("");
  const [isStreaming, setIsStreaming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const abortRef = useRef<AbortController | null>(null);

  const cancel = useCallback(() => {
    abortRef.current?.abort();
    abortRef.current = null;
    setIsStreaming(false);
  }, []);

  const generate = useCallback(
    async (segments: Segment[]) => {
      cancel();

      setSummary("");
      setError(null);
      setIsStreaming(true);

      const controller = new AbortController();
      abortRef.current = controller;

      try {
        const token = await getAccessToken();

        const res = await fetch(`${env.VITE_API_URL}/llm/chat/completions`, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            Authorization: `Bearer ${token}`,
          },
          body: JSON.stringify({
            messages: [
              { role: "system", content: SYSTEM_PROMPT },
              { role: "user", content: buildUserPrompt(segments) },
            ],
            stream: true,
          }),
          signal: controller.signal,
        });

        if (!res.ok) {
          const text = await res.text();
          throw new Error(text || `HTTP ${res.status}`);
        }

        const reader = res.body?.getReader();
        if (!reader) {
          throw new Error("No response body");
        }

        const decoder = new TextDecoder();
        let accumulated = "";
        let buffer = "";

        while (true) {
          const { done, value } = await reader.read();
          if (done) break;

          buffer += decoder.decode(value, { stream: true });
          const lines = buffer.split("\n");
          buffer = lines.pop() ?? "";

          for (const line of lines) {
            const trimmed = line.trim();
            if (!trimmed || !trimmed.startsWith("data: ")) continue;

            const payload = trimmed.slice(6);
            if (payload === "[DONE]") break;

            try {
              const json = JSON.parse(payload);
              const delta = json.choices?.[0]?.delta?.content;
              if (typeof delta === "string") {
                accumulated += delta;
                setSummary(accumulated);
              }
            } catch {
              // skip malformed chunks
            }
          }
        }

        setIsStreaming(false);
      } catch (e) {
        if ((e as Error).name === "AbortError") return;
        setError(e instanceof Error ? e.message : "Summary generation failed");
        setIsStreaming(false);
      }
    },
    [cancel],
  );

  return { summary, isStreaming, error, generate, cancel };
}
