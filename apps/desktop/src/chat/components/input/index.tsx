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
  hasContextBar,
  isStreaming,
  onStop,
  onContextRefsChange,
}: {
  draftKey: string;
  onSendMessage: (
    content: string,
    parts: Array<{ type: "text"; text: string }>,
    contextRefs?: ContextRef[],
  ) => void;
  disabled?: boolean | { disabled: boolean; message?: string };
  hasContextBar?: boolean;
  isStreaming?: boolean;
  onStop?: () => void;
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
    onContextRefsChange,
  });
  const handleSubmit = useSubmit({
    draftKey,
    editorRef,
    disabled,
    isStreaming,
    onSendMessage,
    onContextRefsChange,
  });
  useAutoFocusEditor({ editorRef, disabled, shouldFocus });
  const mentionConfig = useMentionConfig();
  const isSendDisabled = Boolean(disabled) || !hasContent;
  const isRightPanel = chat.mode === "RightPanelOpen";

  return (
    <Container
      elevatedSurfaceClassName={elevatedSurfaceClassName}
      hasContextBar={hasContextBar}
      isRightPanel={isRightPanel}
    >
      <div data-chat-message-input className="flex flex-col px-2 pt-3 pb-2">
        <div className="mb-1 min-h-0 flex-1">
          <ChatEditor
            ref={editorRef}
            className={cn([
              "chat-input-editor",
              "max-h-[40vh] overflow-y-auto overscroll-contain text-sm",
            ])}
            initialContent={initialContent}
            mentionConfig={mentionConfig}
            placeholder={chatPlaceholder}
            onUpdate={handleEditorUpdate}
            onSubmit={handleSubmit}
          />
        </div>

        <div className="flex shrink-0 items-center justify-between">
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
      </div>
    </Container>
  );
}

function Container({
  children,
  elevatedSurfaceClassName,
  hasContextBar,
  isRightPanel,
}: {
  children: React.ReactNode;
  elevatedSurfaceClassName: string;
  hasContextBar?: boolean;
  isRightPanel: boolean;
}) {
  return (
    <div
      className={cn([
        "relative min-w-0 shrink-0",
        isRightPanel ? "px-3 pb-4" : "px-2 pb-2",
      ])}
    >
      <div
        data-chat-input-surface="elevated"
        className={cn([
          "flex max-h-full flex-col border",
          elevatedSurfaceClassName,
          hasContextBar ? "rounded-t-none rounded-b-xl" : "rounded-xl",
          hasContextBar && "border-t-0",
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
