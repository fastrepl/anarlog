import type { ChatStatus } from "ai";
import { tool } from "ai";
import { ChevronDownIcon, SparklesIcon, WandSparklesIcon } from "lucide-react";
import { useCallback, useMemo } from "react";
import { z } from "zod";

import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import { useLanguageModel } from "~/ai/hooks";
import type { TaskType } from "~/ai/prompts/config";
import { ChatBodyNonEmpty } from "~/chat/components/body/non-empty";
import { useChatAutoScroll } from "~/chat/components/body/use-chat-auto-scroll";
import { ChatMessageInput } from "~/chat/components/input";
import { ChatSession } from "~/chat/components/session-provider";
import type { HyprUIMessage } from "~/chat/types";
import { id } from "~/shared/utils";

const PROMPT_ASSISTANT_SUGGESTIONS = {
  enhance: [
    "Make this prompt more action-item focused.",
    "Tighten the structure so the instructions feel shorter and clearer.",
    "Rewrite this so the output reads more executive and less verbose.",
  ],
  title: [
    "Make the title guidance punchier and less generic.",
    "Bias the title toward decisions instead of broad meeting names.",
    "Shorten the title output to four or five words max.",
  ],
} satisfies Record<TaskType, string[]>;

export function PromptAssistantPanel({
  selectedTask,
  taskLabel,
  taskDescription,
  variables,
  filters,
  draftContent,
  hasCustomPrompt,
  onApplyTemplate,
}: {
  selectedTask: TaskType;
  taskLabel: string;
  taskDescription: string;
  variables: string[];
  filters: string[];
  draftContent: string;
  hasCustomPrompt: boolean;
  onApplyTemplate: (content: string) => void;
}) {
  const model = useLanguageModel("chat");
  const sessionId = useMemo(() => id(), [selectedTask]);

  const assistantPrompt = useMemo(
    () =>
      [
        "You are helping the user edit a custom Jinja template for Char.",
        `Task: ${taskLabel}`,
        `Description: ${taskDescription}`,
        "",
        "This editor controls the custom override surface rendered with renderCustom(...).",
        "Do not refer to internal Askama macros or hidden template helpers.",
        "",
        `Saved state: ${hasCustomPrompt ? "custom override" : "default behavior with no override saved yet"}`,
        `Available variables: ${variables.join(", ") || "none"}`,
        `Available filters: ${filters.join(", ") || "none"}`,
        "",
        "Rules:",
        "- Keep responses concise.",
        "- Explain changes briefly before or after applying them.",
        "- When the user asks for a concrete change, call update_prompt_template with the full next template.",
        "- Do not say the draft was updated unless you actually call the tool.",
        "- Preserve valid Jinja syntax.",
        "",
        "<current_template>",
        draftContent,
        "</current_template>",
      ].join("\n"),
    [
      draftContent,
      filters,
      hasCustomPrompt,
      taskDescription,
      taskLabel,
      variables,
    ],
  );

  const extraTools = useMemo(
    () => ({
      update_prompt_template: tool({
        description:
          "Replace the current prompt draft with a complete updated Jinja template.",
        inputSchema: z.object({
          content: z
            .string()
            .describe("The full updated prompt template in Jinja syntax."),
          summary: z
            .string()
            .optional()
            .describe("A short note about what changed in the draft."),
        }),
        execute: async ({
          content,
          summary,
        }: {
          content: string;
          summary?: string;
        }) => {
          const nextContent = content.trim();
          onApplyTemplate(nextContent);

          return {
            status: "applied",
            message:
              summary ??
              "Draft updated in the editor. Review and save to make it live.",
            lineCount: nextContent.split("\n").length,
          };
        },
      }),
    }),
    [onApplyTemplate],
  );

  const handleSendMessage = useCallback(
    (
      _content: string,
      parts: HyprUIMessage["parts"],
      sendMessage: (message: HyprUIMessage) => void,
    ) => {
      sendMessage({
        id: id(),
        role: "user",
        parts,
        metadata: {
          createdAt: Date.now(),
        },
      });
    },
    [],
  );

  return (
    <div className="flex h-full min-h-0 flex-col bg-stone-50">
      <div className="border-b border-neutral-200 px-4 py-4">
        <div className="flex items-center gap-2 text-sm font-medium text-neutral-900">
          <WandSparklesIcon className="h-4 w-4 text-neutral-500" />
          Prompt Assistant
        </div>
        <p className="mt-1 text-xs leading-5 text-neutral-600">
          Ask Charlie to rewrite the draft, tighten the language, or reshape the
          structure. Applied changes land back in the editor so you can review
          them before saving.
        </p>
      </div>

      <ChatSession
        key={selectedTask}
        sessionId={sessionId}
        modelOverride={model ?? undefined}
        extraTools={extraTools}
        systemPromptOverride={assistantPrompt}
      >
        {(sessionProps) => (
          <div className="flex min-h-0 flex-1 flex-col">
            <PromptAssistantBody
              messages={sessionProps.messages}
              status={sessionProps.status}
              error={sessionProps.error}
              regenerate={sessionProps.regenerate}
              isModelConfigured={!!model}
              selectedTask={selectedTask}
              onSendPrompt={(prompt) => {
                handleSendMessage(
                  prompt,
                  [{ type: "text", text: prompt }],
                  sessionProps.sendMessage,
                );
              }}
            />

            {model ? (
              <ChatMessageInput
                draftKey={sessionProps.sessionId}
                disabled={!sessionProps.isSystemPromptReady}
                onSendMessage={(content, parts) => {
                  handleSendMessage(content, parts, sessionProps.sendMessage);
                }}
                isStreaming={
                  sessionProps.status === "streaming" ||
                  sessionProps.status === "submitted"
                }
                onStop={sessionProps.stop}
              />
            ) : (
              <div className="border-t border-neutral-200 px-4 py-3 text-xs text-neutral-500">
                Configure a chat model in AI settings to edit prompts from chat.
              </div>
            )}
          </div>
        )}
      </ChatSession>
    </div>
  );
}

function PromptAssistantBody({
  messages,
  status,
  error,
  regenerate,
  isModelConfigured,
  selectedTask,
  onSendPrompt,
}: {
  messages: HyprUIMessage[];
  status: ChatStatus;
  error?: Error;
  regenerate: () => void;
  isModelConfigured: boolean;
  selectedTask: TaskType;
  onSendPrompt: (prompt: string) => void;
}) {
  const {
    contentRef,
    isAtBottom,
    scrollRef,
    scrollToBottom,
    showGoToRecent,
    updateAutoScrollState,
    handleWheel,
  } = useChatAutoScroll(status);

  return (
    <div className="relative flex min-h-0 flex-1 flex-col">
      <div
        ref={scrollRef}
        onScroll={updateAutoScrollState}
        onWheel={handleWheel}
        className="flex min-h-0 flex-1 flex-col overflow-y-auto"
      >
        <div
          ref={contentRef}
          className="flex min-h-full flex-1 flex-col px-3 py-3"
        >
          <div className="flex-1" />
          {messages.length === 0 ? (
            <PromptAssistantEmpty
              isModelConfigured={isModelConfigured}
              selectedTask={selectedTask}
              onSendPrompt={onSendPrompt}
            />
          ) : (
            <ChatBodyNonEmpty
              messages={messages}
              status={status}
              error={error}
              onReload={regenerate}
            />
          )}
        </div>
      </div>

      {messages.length > 0 && showGoToRecent && !isAtBottom ? (
        <Button
          onClick={scrollToBottom}
          size="sm"
          className="absolute bottom-3 left-1/2 z-20 flex -translate-x-1/2 items-center gap-1 rounded-full border border-neutral-200 bg-white text-neutral-700 shadow-xs hover:bg-neutral-50"
          variant="outline"
        >
          <ChevronDownIcon size={12} />
          <span className="text-xs">Go to recent</span>
        </Button>
      ) : null}
    </div>
  );
}

function PromptAssistantEmpty({
  isModelConfigured,
  selectedTask,
  onSendPrompt,
}: {
  isModelConfigured: boolean;
  selectedTask: TaskType;
  onSendPrompt: (prompt: string) => void;
}) {
  return (
    <div className="flex justify-start pb-1">
      <div className="flex w-full flex-col">
        <div className="mb-2 flex items-center gap-2">
          <SparklesIcon className="h-4 w-4 text-neutral-500" />
          <span className="text-sm font-medium text-neutral-800">Charlie</span>
        </div>
        <p className="mb-3 text-sm leading-6 text-neutral-700">
          {isModelConfigured
            ? "I can rewrite the active draft, preserve the Jinja structure, and apply the updated template back into the editor."
            : "Set up a chat model to rewrite the active draft from this pane."}
        </p>
        {isModelConfigured ? (
          <div className="flex flex-wrap gap-1.5">
            {PROMPT_ASSISTANT_SUGGESTIONS[selectedTask].map((prompt) => (
              <button
                key={prompt}
                type="button"
                onClick={() => onSendPrompt(prompt)}
                className={cn([
                  "rounded-full border border-neutral-300 bg-white px-2.5 py-1 text-left text-[11px] text-neutral-700",
                  "transition-colors hover:bg-neutral-100",
                ])}
              >
                {prompt}
              </button>
            ))}
          </div>
        ) : null}
      </div>
    </div>
  );
}
