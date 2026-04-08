import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  PlayIcon,
  RefreshCwIcon,
  SparklesIcon,
  SquareIcon,
} from "lucide-react";
import { useState } from "react";

import { Badge } from "@hypr/ui/components/ui/badge";
import { Button } from "@hypr/ui/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@hypr/ui/components/ui/card";
import { Label } from "@hypr/ui/components/ui/label";
import { Textarea } from "@hypr/ui/components/ui/textarea";
import { sonnerToast } from "@hypr/ui/components/ui/toast";
import { cn } from "@hypr/utils";

import { commands as localLlmCommands } from "../../../../../plugins/local-llm/js";

type ChatCompletionResponse = {
  choices?: Array<{
    message?: { content?: string | Array<{ type?: string; text?: string }> };
  }>;
};

function extractContent(response: ChatCompletionResponse) {
  const content = response.choices?.[0]?.message?.content;
  if (typeof content === "string") return content;
  if (Array.isArray(content)) {
    return (
      content
        .filter((p) => p.type === "text" && typeof p.text === "string")
        .map((p) => p.text?.trim() ?? "")
        .filter(Boolean)
        .join("\n\n") || JSON.stringify(content, null, 2)
    );
  }
  return "";
}

export function LocalLlmTester() {
  const queryClient = useQueryClient();
  const [response, setResponse] = useState("");

  const serverUrlQuery = useQuery({
    queryKey: ["local-llm", "server-url"],
    queryFn: async () => {
      const result = await localLlmCommands.serverUrl();
      if (result.status === "error") throw new Error(result.error);
      return result.data;
    },
    refetchInterval: 5_000,
  });

  const startServerMutation = useMutation({
    mutationFn: async () => {
      const selectionResult = await localLlmCommands.setCurrentModelSelection({
        type: "Cactus",
        content: {
          key: "cactus-lfm2-vl-450m-apple",
        },
      });
      if (selectionResult.status === "error") {
        throw new Error(selectionResult.error);
      }

      const result = await localLlmCommands.startServer();
      if (result.status === "error") throw new Error(result.error);
      return result.data;
    },
    onSuccess: (url) => {
      queryClient.setQueryData(["local-llm", "server-url"], url);
    },
    onError: (error) => sonnerToast.error(error.message),
  });

  const stopServerMutation = useMutation({
    mutationFn: async () => {
      const result = await localLlmCommands.stopServer();
      if (result.status === "error") throw new Error(result.error);
    },
    onSuccess: () => {
      queryClient.setQueryData(["local-llm", "server-url"], null);
    },
    onError: (error) => sonnerToast.error(error.message),
  });

  const form = useForm({
    defaultValues: {
      prompt: "Write one sentence confirming the embedded model is working.",
      temperature: "0.2",
      maxTokens: "256",
    },
    onSubmit: async ({ value }) => {
      try {
        const baseUrl = serverUrlQuery.data;
        if (!baseUrl) throw new Error("Start the server first.");

        setResponse("");

        const res = await fetch(`${baseUrl}/chat/completions`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            messages: [{ role: "user", content: value.prompt }],
            temperature: Number(value.temperature),
            max_tokens: Number(value.maxTokens),
          }),
        });

        if (!res.ok) throw new Error(await res.text());

        const content = extractContent(
          (await res.json()) as ChatCompletionResponse,
        );
        if (!content) {
          throw new Error("Model returned an empty response.");
        }

        setResponse(content);
      } catch (error) {
        const message =
          error instanceof Error ? error.message : "Request failed";
        sonnerToast.error(message);
        throw error;
      }
    },
  });

  const isRunning = Boolean(serverUrlQuery.data);
  const busy = startServerMutation.isPending || stopServerMutation.isPending;

  return (
    <Card variant="outline-solid">
      <CardHeader spacing="compact">
        <div className="flex items-center justify-between gap-4">
          <CardTitle className="text-sm">Embedded LLM</CardTitle>
          <div className="flex items-center gap-2">
            <span className="text-xs text-neutral-500">
              {serverUrlQuery.data ?? ""}
            </span>
            <Badge variant={isRunning ? "success" : "outline"}>
              {isRunning ? "Running" : "Stopped"}
            </Badge>
          </div>
        </div>
      </CardHeader>
      <CardContent spacing="compact" className="flex flex-col gap-4">
        <div className="flex items-center gap-2">
          <Button
            type="button"
            size="sm"
            onClick={() => startServerMutation.mutate()}
            disabled={busy || isRunning}
          >
            <PlayIcon className="mr-2 h-4 w-4" />
            Start
          </Button>
          <Button
            type="button"
            size="sm"
            variant="outline"
            onClick={() => stopServerMutation.mutate()}
            disabled={busy || !isRunning}
          >
            <SquareIcon className="mr-2 h-4 w-4" />
            Stop
          </Button>
          <Button
            type="button"
            size="sm"
            variant="ghost"
            onClick={() =>
              void queryClient.invalidateQueries({ queryKey: ["local-llm"] })
            }
            disabled={serverUrlQuery.isFetching}
          >
            <RefreshCwIcon
              className={cn([
                "h-4 w-4",
                serverUrlQuery.isFetching && "animate-spin",
              ])}
            />
          </Button>
        </div>

        <form
          className="flex flex-col gap-3"
          onSubmit={(e) => {
            e.preventDefault();
            e.stopPropagation();
            void form.handleSubmit();
          }}
        >
          <form.Field name="prompt">
            {(field) => (
              <Textarea
                id={field.name}
                value={field.state.value}
                onChange={(e) => field.handleChange(e.target.value)}
                rows={3}
                placeholder="Prompt"
              />
            )}
          </form.Field>

          <div className="flex items-center gap-3">
            <form.Field name="temperature">
              {(field) => (
                <div className="flex items-center gap-2">
                  <Label
                    htmlFor={field.name}
                    className="text-xs text-neutral-500"
                  >
                    temp
                  </Label>
                  <input
                    id={field.name}
                    value={field.state.value}
                    onChange={(e) => field.handleChange(e.target.value)}
                    inputMode="decimal"
                    className="w-14 rounded border border-neutral-200 bg-transparent px-2 py-1 text-xs"
                  />
                </div>
              )}
            </form.Field>

            <form.Field name="maxTokens">
              {(field) => (
                <div className="flex items-center gap-2">
                  <Label
                    htmlFor={field.name}
                    className="text-xs text-neutral-500"
                  >
                    max tokens
                  </Label>
                  <input
                    id={field.name}
                    value={field.state.value}
                    onChange={(e) => field.handleChange(e.target.value)}
                    inputMode="numeric"
                    className="w-16 rounded border border-neutral-200 bg-transparent px-2 py-1 text-xs"
                  />
                </div>
              )}
            </form.Field>

            <Button
              type="submit"
              size="sm"
              className="ml-auto"
              disabled={busy || !isRunning}
            >
              <SparklesIcon className="mr-2 h-4 w-4" />
              {form.state.isSubmitting ? "Running..." : "Send"}
            </Button>
          </div>
        </form>

        {response && (
          <Textarea readOnly value={response} rows={6} className="text-xs" />
        )}
      </CardContent>
    </Card>
  );
}
