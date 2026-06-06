import {
  ArrowUpIcon,
  ArrowUpRightIcon,
  Settings2Icon,
  SparklesIcon,
  XIcon,
} from "lucide-react";
import { useEffect, useRef } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { ChatEditor, type ChatEditorHandle } from "@hypr/editor/chat";
import type { PlaceholderFunction } from "@hypr/editor/plugins";
import { commands as windowsCommands } from "@hypr/plugin-windows";
import { cn } from "@hypr/utils";

import { useLanguageModel } from "~/ai/hooks";
import {
  useAutoFocusEditor,
  useDraftState,
  useSubmit,
} from "~/chat/components/input/hooks";
import { ChatSession } from "~/chat/components/session-provider";
import { dedupeByKey, type ContextRef } from "~/chat/context/entities";
import { useChatActions } from "~/chat/store/use-chat-actions";
import { useShell } from "~/contexts/shell";
import { useMentionConfig } from "~/editor-bridge/mention-config";
import * as main from "~/store/tinybase/store/main";

export function ComposerScreen() {
  const { chat } = useShell();
  const model = useLanguageModel("chat");
  const { user_id } = main.UI.useValues(main.STORE_ID);
  const currentTitle = main.UI.useCell(
    "chat_groups",
    chat.groupId ?? "",
    "title",
    main.STORE_ID,
  );
  const { handleSendMessage } = useChatActions({
    groupId: chat.groupId,
    onGroupCreated: chat.setGroupId,
  });

  useEffect(() => {
    chat.sendEvent({ type: "OPEN" });

    return () => {
      chat.sendEvent({ type: "CLOSE" });
    };
  }, [chat]);

  useHotkeys(
    "esc",
    () => {
      void dismissComposer();
    },
    {
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [],
  );

  if (!user_id) {
    return <div className="h-screen w-screen bg-transparent" />;
  }

  return (
    <div className="h-screen w-screen bg-transparent">
      <ChatSession
        key={chat.sessionId}
        sessionId={chat.sessionId}
        chatGroupId={chat.groupId}
      >
        {(sessionProps) => {
          const sendMessage = (
            content: string,
            parts: Array<{ type: "text"; text: string }>,
            contextRefs?: ContextRef[],
          ) => {
            handleSendMessage(
              content,
              parts,
              sessionProps.sendMessage,
              contextRefs
                ? dedupeByKey([sessionProps.pendingRefs, contextRefs])
                : sessionProps.pendingRefs,
            );
          };

          return model ? (
            <ComposerInput
              draftKey={sessionProps.sessionId}
              disabled={!sessionProps.isSystemPromptReady}
              isStreaming={
                sessionProps.status === "streaming" ||
                sessionProps.status === "submitted"
              }
              onStop={sessionProps.stop}
              onSendMessage={sendMessage}
              title={currentTitle || "Ask Anarlog AI anything"}
            />
          ) : (
            <ComposerSettingsCard />
          );
        }}
      </ChatSession>
    </div>
  );
}

function ComposerSettingsCard() {
  return (
    <div
      className={cn([
        "h-full w-full rounded-[28px] px-5 py-4",
        "bg-primary/88 text-primary-foreground",
      ])}
    >
      <div className="mb-3 flex items-center justify-between gap-3">
        <div data-tauri-drag-region className="min-w-0 flex-1 pr-4">
          <p className="text-primary-foreground/38 text-[10px] font-semibold tracking-[0.24em] uppercase">
            Composer
          </p>
          <p className="text-primary-foreground/72 truncate pt-1 text-sm">
            Configure a chat model to use the quick composer.
          </p>
        </div>

        <button
          type="button"
          onClick={() => void dismissComposer()}
          data-tauri-drag-region="false"
          className={cn([
            "inline-flex size-8 items-center justify-center rounded-full",
            "bg-primary-foreground/7 text-primary-foreground/65 transition-colors",
            "hover:bg-primary-foreground/12 hover:text-primary-foreground",
          ])}
        >
          <XIcon className="size-4" />
        </button>
      </div>

      <button
        type="button"
        onClick={() => void openSettingsInMainWindow()}
        className={cn([
          "inline-flex items-center gap-2 rounded-full px-3.5 py-2 text-sm font-medium",
          "bg-primary-foreground/7 text-primary-foreground/85 transition-colors",
          "hover:bg-primary-foreground/10 hover:text-primary-foreground",
        ])}
      >
        <Settings2Icon className="size-4" />
        Configure a chat model in Settings
      </button>
    </div>
  );
}

function ComposerInput({
  draftKey,
  disabled,
  isStreaming,
  onStop,
  onSendMessage,
  title,
}: {
  draftKey: string;
  disabled?: boolean;
  isStreaming?: boolean;
  onStop?: () => void;
  title: string;
  onSendMessage: (
    content: string,
    parts: Array<{ type: "text"; text: string }>,
    contextRefs?: ContextRef[],
  ) => void;
}) {
  const editorRef = useRef<ChatEditorHandle>(null);
  const { hasContent, initialContent, handleEditorUpdate } = useDraftState({
    draftKey,
  });
  const handleSubmit = useSubmit({
    draftKey,
    editorRef,
    disabled,
    isStreaming,
    onSendMessage,
  });
  const mentionConfig = useMentionConfig();

  useAutoFocusEditor({
    editorRef,
    disabled,
  });

  return (
    <div
      className={cn([
        "h-full w-full rounded-[28px] px-5 py-4",
        "bg-primary/88 text-primary-foreground",
      ])}
    >
      <div className="mb-3 flex items-start justify-between gap-4">
        <div data-tauri-drag-region className="min-w-0 flex-1 pr-4">
          <p className="text-primary-foreground/38 text-[10px] font-semibold tracking-[0.24em] uppercase">
            Composer
          </p>
          <p className="text-primary-foreground/90 truncate pt-1 text-[15px]">
            {title}
          </p>
        </div>

        <div data-tauri-drag-region="false" className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => void openMainWindow()}
            data-tauri-drag-region="false"
            className={cn([
              "inline-flex items-center gap-1.5 rounded-full px-3 py-1.5 text-xs font-medium",
              "bg-primary-foreground/7 text-primary-foreground/76",
              "hover:bg-primary-foreground/12 hover:text-primary-foreground transition-colors",
            ])}
          >
            <ArrowUpRightIcon className="size-3.5" />
            Open Anarlog
          </button>
          <button
            type="button"
            onClick={() => void dismissComposer()}
            data-tauri-drag-region="false"
            className={cn([
              "inline-flex size-8 items-center justify-center rounded-full",
              "bg-primary-foreground/7 text-primary-foreground/65 transition-colors",
              "hover:bg-primary-foreground/12 hover:text-primary-foreground",
            ])}
          >
            <XIcon className="size-4" />
          </button>
        </div>
      </div>

      <ChatEditor
        ref={editorRef}
        className={cn([
          "text-primary-foreground max-h-[88px] min-h-[34px] overflow-y-auto text-[15px] leading-6",
          "[&_.ProseMirror]:min-h-[34px] [&_.ProseMirror]:outline-none",
          "[&_.ProseMirror]:placeholder:text-primary-foreground/28",
        ])}
        initialContent={initialContent}
        mentionConfig={mentionConfig}
        placeholder={composerPlaceholder}
        onUpdate={handleEditorUpdate}
        onSubmit={handleSubmit}
      />

      <div className="mt-3 flex items-center justify-between gap-3">
        <div className="text-primary-foreground/40 flex items-center gap-2 text-[11px]">
          <span className="bg-primary-foreground/8 rounded-full px-2 py-1">
            Esc to dismiss
          </span>
          <span className="bg-primary-foreground/8 rounded-full px-2 py-1">
            ⌘ ↩ to send
          </span>
        </div>

        {isStreaming ? (
          <button
            type="button"
            onClick={onStop}
            className={cn([
              "inline-flex items-center gap-2 rounded-full px-3 py-1.5 text-xs font-medium",
              "bg-primary-foreground/8 text-primary-foreground/82 transition-colors",
              "hover:bg-primary-foreground/12 hover:text-primary-foreground",
            ])}
          >
            <SparklesIcon className="size-3.5" />
            Stop
          </button>
        ) : (
          <button
            type="button"
            onClick={handleSubmit}
            disabled={disabled}
            className={cn([
              "inline-flex size-10 items-center justify-center rounded-full",
              disabled
                ? "bg-primary-foreground/8 text-primary-foreground/25 cursor-default"
                : [
                    "bg-primary-foreground text-primary",
                    "transition-transform hover:scale-[1.02]",
                  ],
              !hasContent && !disabled && "opacity-55",
            ])}
          >
            <ArrowUpIcon className="size-4" />
          </button>
        )}
      </div>
    </div>
  );
}

const composerPlaceholder: PlaceholderFunction = ({ node, pos }) => {
  if (node.type.name === "paragraph" && pos === 0) {
    return "Message Anarlog AI";
  }

  return "";
};

async function openMainWindow() {
  await windowsCommands.windowShow({ type: "main" });
  await dismissComposer();
}

async function openSettingsInMainWindow() {
  await windowsCommands.windowShow({ type: "main" });
  await windowsCommands.windowEmitNavigate(
    { type: "main" },
    { path: "/app/settings", search: { tab: "intelligence" } },
  );
  await dismissComposer();
}

async function dismissComposer() {
  const result = await windowsCommands.windowHide({ type: "composer" });

  if (result.status === "error") {
    console.error("Failed to dismiss composer:", result.error);
  }
}
