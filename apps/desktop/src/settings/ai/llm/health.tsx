import { useQuery } from "@tanstack/react-query";
import { generateText } from "ai";
import { useEffect } from "react";

import { Spinner } from "@anlg/ui/components/ui/spinner";

import { useLanguageModel } from "~/ai/hooks";

export type LlmHealthStatus = {
  status: "pending" | "error" | "success" | null;
  message?: string;
};

export function HealthStatusIndicator() {
  const health = useConnectionHealth();

  if (health.status === "pending") {
    return <Spinner size={14} className="text-muted-foreground shrink-0" />;
  }

  return null;
}

export function useConnectionHealth(): LlmHealthStatus {
  const model = useLanguageModel();

  const text = useQuery({
    enabled: !!model,
    queryKey: ["llm-health-check", model],
    staleTime: 0,
    retry: 5,
    retryDelay: 200,
    queryFn: async () => {
      const result = await generateText({
        model: model!,
        system: "If user says hi, respond with hello, without any other text.",
        prompt: "Hi",
      });
      return result;
    },
  });

  const { refetch } = text;
  useEffect(() => {
    if (model) {
      void refetch();
    }
  }, [model, refetch]);

  if (!model) {
    return { status: null };
  }

  if (text.status === "error") {
    return {
      status: "error",
      message: `Connection failed: ${llmHealthErrorMessage(text.error)}`,
    };
  }

  return { status: text.status };
}

export function llmHealthErrorMessage(error: unknown): string {
  if (!error || typeof error !== "object") {
    return "Unknown error";
  }

  const api = error as {
    message?: unknown;
    data?: unknown;
    responseBody?: unknown;
  };
  const fromPayload =
    apiErrorFromUnknown(api.data) ??
    (typeof api.responseBody === "string"
      ? (apiErrorFromUnknown(tryJson(api.responseBody)) ??
        firstUsefulLine(api.responseBody))
      : undefined);
  if (fromPayload) {
    return fromPayload;
  }
  if (typeof api.message === "string" && api.message.trim()) {
    return firstUsefulLine(api.message) || "Unknown error";
  }

  return "Unknown error";
}

function apiErrorFromUnknown(value: unknown): string | undefined {
  if (!value || typeof value !== "object") {
    return undefined;
  }

  const record = value as Record<string, unknown>;
  if (typeof record.message === "string" && record.message.trim()) {
    return firstUsefulLine(record.message);
  }
  if (typeof record.detail === "string" && record.detail.trim()) {
    return firstUsefulLine(record.detail);
  }

  const nested = record.error;
  if (typeof nested === "string" && nested.trim()) {
    return firstUsefulLine(nested);
  }
  if (nested && typeof nested === "object") {
    const nestedRecord = nested as Record<string, unknown>;
    if (
      typeof nestedRecord.message === "string" &&
      nestedRecord.message.trim()
    ) {
      return firstUsefulLine(nestedRecord.message);
    }
  }

  return undefined;
}

function tryJson(value: string): unknown {
  try {
    return JSON.parse(value);
  } catch {
    return undefined;
  }
}

function firstUsefulLine(value: string): string {
  const line =
    value
      .split("\n")
      .map((part) => part.trim())
      .find((part) => part.length > 0) ?? value.trim();
  return line.length > 200 ? `${line.slice(0, 197)}...` : line;
}
