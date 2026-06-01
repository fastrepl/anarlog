import { SquareIcon } from "lucide-react";
import { useRef } from "react";

import { ChatEditor, type ChatEditorHandle } from "@hypr/editor/chat";
import type { PlaceholderFunction } from "@hypr/editor/plugins";
import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import { useAutoFocusEditor, useDraftState, useSubmit } from "./hooks";

import type { ContextRef } from "~/chat/context/entities";
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
    <Container hasContextBar={hasContextBar} isRightPanel={isRightPanel}>
      <div data-chat-message-input className="flex flex-col px-2 pt-3 pb-2">
        <div className="mb-1 min-h-0 flex-1">
          <ChatEditor
            ref={editorRef}
            className="max-h-[40vh] overflow-y-auto overscroll-contain text-sm text-neutral-900"
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
                "inline-flex h-7 items-center gap-1.5 rounded-lg pr-1.5 pl-2.5 text-xs font-medium transition-all duration-100",
                "border",
                isSendDisabled
                  ? "cursor-default border-neutral-200 text-neutral-300"
                  : [
                      "border-stone-600 bg-stone-800 text-white",
                      "hover:bg-stone-700",
                      "active:scale-[0.97] active:bg-stone-600",
                    ],
              ])}
            >
              Send
              <span
                className={cn([
                  "font-mono text-xs",
                  isSendDisabled ? "text-neutral-300" : "text-stone-400",
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
  hasContextBar,
  isRightPanel,
}: {
  children: React.ReactNode;
  hasContextBar?: boolean;
  isRightPanel: boolean;
}) {
  return (
    <div
      className={cn([
        "relative min-w-0 shrink-0",
        isRightPanel ? "px-3 pb-5" : "px-2 pb-2",
      ])}
    >
      <div
        className={cn([
          "flex max-h-full flex-col border border-neutral-200 bg-white",
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
