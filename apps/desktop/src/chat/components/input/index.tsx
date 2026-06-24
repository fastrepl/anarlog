import "./chat-input.css";

import { useLingui } from "@lingui/react/macro";
import { ArrowUpIcon, SquareIcon } from "lucide-react";
import { useMemo, useRef } from "react";

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
  const { t } = useLingui();
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
  const placeholderText = isFloating
    ? t`Ask anything`
    : t`Ask & search about anything, or be creative!`;
  const placeholderTextRef = useRef(placeholderText);
  placeholderTextRef.current = placeholderText;
  const placeholder = useMemo(
    () => createChatPlaceholder(() => placeholderTextRef.current),
    [],
  );

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
            ? "relative flex max-h-full min-h-[30px] w-full min-w-0 items-center"
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
                ? "max-h-36 min-h-5 w-full min-w-0 overflow-y-auto overscroll-contain"
                : "overflow-y-auto overscroll-contain",
              !isFloating && (isRightPanel ? "max-h-[40vh]" : "max-h-48"),
            ])}
            initialContent={initialContent}
            mentionConfig={mentionConfig}
            placeholder={placeholder}
            onUpdate={handleEditorUpdate}
            onSubmit={handleSubmit}
          />
        </div>

        {showSendControl && (
          <div
            className={cn([
              "flex shrink-0 items-center",
              isFloating
                ? "absolute right-0 bottom-0.5 z-10"
                : "justify-between",
            ])}
          >
            <div />
            {isStreaming ? (
              <Button
                onClick={onStop}
                size="icon"
                variant="ghost"
                className="h-7 w-7 rounded-full"
                aria-label={t`Stop response`}
              >
                <SquareIcon size={14} className="fill-current" />
              </Button>
            ) : (
              <button
                type="button"
                aria-label={t`Send message`}
                onClick={handleSubmit}
                disabled={isSendDisabled}
                className={cn([
                  "chat-input-send",
                  "border-border text-muted-foreground/60 inline-flex size-7 shrink-0 items-center justify-center rounded-full border transition-all duration-100",
                  !isSendDisabled && [
                    "bg-primary text-primary-foreground border-stone-600",
                    "hover:bg-primary/90",
                    "active:bg-primary/80 active:scale-[0.97]",
                  ],
                ])}
              >
                <ArrowUpIcon size={15} strokeWidth={2.25} />
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
                "border-border/70 text-card-foreground max-h-40 min-h-[38px] flex-row items-center overflow-hidden rounded-[19px] bg-white py-[3px] pr-[6px] pl-4 text-sm shadow-none",
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

function createChatPlaceholder(
  getPlaceholder: () => string,
): PlaceholderFunction {
  return ({ node, pos }) => {
    if (node.type.name === "paragraph" && pos === 0) {
      return getPlaceholder();
    }
    return "";
  };
}
