import "./chat-input.css";

import { SquareIcon } from "lucide-react";
import { useRef } from "react";

import { ChatEditor, type ChatEditorHandle } from "@hypr/editor/chat";
import type { PlaceholderFunction } from "@hypr/editor/plugins";
import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import { useAutoFocusEditor, useDraftState, useSubmit } from "./hooks";

import type { ContextRef } from "~/chat/context/entities";
import { useChatAppearance } from "~/chat/hooks/use-chat-appearance";
import { useShell } from "~/contexts/shell";
import { useMentionConfig } from "~/editor-bridge/mention-config";

export function ChatMessageInput({
  draftKey,
  onSendMessage,
  disabled: disabledProp,
  isStreaming,
  onStop,
  onDraftContentChange,
  onContextRefsChange,
}: {
  draftKey: string;
  onSendMessage: (
    content: string,
    parts: Array<{ type: "text"; text: string }>,
    contextRefs?: ContextRef[],
  ) => void;
  disabled?: boolean | { disabled: boolean; message?: string };
  isStreaming?: boolean;
  onStop?: () => void;
  onDraftContentChange?: (hasDraftContent: boolean) => void;
  onContextRefsChange?: (refs: ContextRef[]) => void;
}) {
  const { chat } = useShell();
  const { elevatedSurfaceClassName } = useChatAppearance();
  const editorRef = useRef<ChatEditorHandle>(null);
  const disabled =
    typeof disabledProp === "object" ? disabledProp.disabled : disabledProp;
  const shouldFocus = chat.mode !== "FloatingClosed";

  const { hasContent, initialContent, handleEditorUpdate } = useDraftState({
    draftKey,
    onDraftContentChange,
    onContextRefsChange,
  });
  const handleSubmit = useSubmit({
    draftKey,
    editorRef,
    disabled,
    isStreaming,
    onSendMessage,
    onDraftContentChange,
    onContextRefsChange,
  });
  useAutoFocusEditor({ editorRef, disabled, shouldFocus });
  const mentionConfig = useMentionConfig();
  const isSendDisabled = Boolean(disabled) || !hasContent;
  const isRightPanel = chat.mode === "RightPanelOpen";
  const isFloating = chat.mode === "FloatingOpen";
  const showSendControl = !isFloating || isStreaming || hasContent;

  return (
    <Container
      elevatedSurfaceClassName={elevatedSurfaceClassName}
      isFloating={isFloating}
      isRightPanel={isRightPanel}
    >
      <div
        data-chat-message-input
        className={cn([
          isFloating
            ? "flex h-full min-h-0 w-full min-w-0 items-center"
            : "flex flex-col px-2 pt-3 pb-2",
        ])}
      >
        <div className={cn([isFloating ? "min-w-0 flex-1" : "mb-1 min-h-0"])}>
          <ChatEditor
            ref={editorRef}
            className={cn([
              "chat-input-editor",
              "text-sm",
              isFloating
                ? "max-h-24 w-full min-w-0 overflow-y-auto overscroll-contain"
                : "overflow-y-auto overscroll-contain",
              !isFloating && (isRightPanel ? "max-h-[40vh]" : "max-h-48"),
            ])}
            initialContent={initialContent}
            mentionConfig={mentionConfig}
            placeholder={isFloating ? floatingChatPlaceholder : chatPlaceholder}
            onUpdate={handleEditorUpdate}
            onSubmit={handleSubmit}
          />
        </div>

        {showSendControl && (
          <div
            className={cn([
              "flex shrink-0 items-center",
              isFloating ? "ml-3" : "justify-between",
            ])}
          >
            <div />
            {isStreaming ? (
              <Button
                onClick={onStop}
                size="icon"
                variant="ghost"
                className="h-7 w-7 rounded-full"
              >
                <SquareIcon size={14} className="fill-current" />
              </Button>
            ) : (
              <button
                onClick={handleSubmit}
                disabled={isSendDisabled}
                className={cn([
                  "chat-input-send",
                  "inline-flex h-7 items-center gap-1.5 rounded-lg border pr-1.5 pl-2.5 text-xs font-medium transition-all duration-100",
                  !isSendDisabled && [
                    "bg-primary text-primary-foreground border-stone-600",
                    "hover:bg-primary/90",
                    "active:bg-primary/80 active:scale-[0.97]",
                  ],
                ])}
              >
                Send
                <span
                  className={cn([
                    "chat-input-send-shortcut font-mono text-xs",
                    !isSendDisabled && "text-stone-400",
                  ])}
                >
                  ⌘ ↩
                </span>
              </button>
            )}
          </div>
        )}
      </div>
    </Container>
  );
}

function Container({
  children,
  elevatedSurfaceClassName,
  isFloating,
  isRightPanel,
}: {
  children: React.ReactNode;
  elevatedSurfaceClassName: string;
  isFloating: boolean;
  isRightPanel: boolean;
}) {
  return (
    <div
      className={cn([
        "relative min-w-0 shrink-0",
        isRightPanel ? "px-3 pb-4" : "px-1 pb-1",
      ])}
    >
      <div
        data-chat-input-surface={isFloating ? "floating" : "elevated"}
        className={cn([
          "flex max-h-full border",
          isFloating
            ? [
                "border-border/70 text-card-foreground h-10 max-h-10 min-h-10 flex-row items-center overflow-hidden rounded-full bg-white px-4 py-0 text-sm shadow-none",
                "dark:bg-card dark:text-card-foreground",
              ]
            : [elevatedSurfaceClassName, "flex-col rounded-xl"],
        ])}
      >
        {children}
      </div>
    </div>
  );
}

const chatPlaceholder: PlaceholderFunction = ({ node, pos }) => {
  if (node.type.name === "paragraph" && pos === 0) {
    return "Ask & search about anything, or be creative!";
  }
  return "";
};

const floatingChatPlaceholder: PlaceholderFunction = ({ node, pos }) => {
  if (node.type.name === "paragraph" && pos === 0) {
    return "Ask anything";
  }
  return "";
};
