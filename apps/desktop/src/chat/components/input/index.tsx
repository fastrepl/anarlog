import { SquareIcon } from "lucide-react";
import { useRef } from "react";

import type { TiptapEditor } from "@hypr/tiptap/chat";
import ChatEditor from "@hypr/tiptap/chat";
import type { PlaceholderFunction } from "@hypr/tiptap/shared";
import { Button } from "@hypr/ui/components/ui/button";
import { cn } from "@hypr/utils";

import {
  useAutoFocusEditor,
  useDraftState,
  useSlashCommandConfig,
  useSubmit,
} from "./hooks";
import { type McpIndicator, McpIndicatorBadge } from "./mcp";

import { useShell } from "~/contexts/shell";

export type { McpIndicator } from "./mcp";

export function ChatMessageInput({
  draftKey,
  onSendMessage,
  disabled: disabledProp,
  hasContextBar,
  isStreaming,
  onStop,
  mcpIndicator,
}: {
  draftKey: string;
  onSendMessage: (
    content: string,
    parts: Array<{ type: "text"; text: string }>,
  ) => void;
  disabled?: boolean | { disabled: boolean; message?: string };
  hasContextBar?: boolean;
  isStreaming?: boolean;
  onStop?: () => void;
  mcpIndicator?: McpIndicator;
}) {
  const { chat } = useShell();
  const editorRef = useRef<{ editor: TiptapEditor | null }>(null);
  const disabled =
    typeof disabledProp === "object" ? disabledProp.disabled : disabledProp;
  const shouldFocus =
    chat.mode === "FloatingOpen" || chat.mode === "RightPanelOpen";

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
  useAutoFocusEditor({ editorRef, disabled, shouldFocus });
  const slashCommandConfig = useSlashCommandConfig();

  return (
    <Container
      hasContextBar={hasContextBar}
      isRightPanel={chat.mode === "RightPanelOpen"}
    >
      <div
        className={cn([
          "flex flex-col pt-3 pb-2",
          chat.mode === "RightPanelOpen" ? "px-2" : "px-2",
        ])}
      >
        <div className="mb-1 min-h-0 flex-1">
          <ChatEditor
            ref={editorRef}
            className="max-h-[40vh] overflow-y-auto overscroll-contain"
            editable={!disabled}
            initialContent={initialContent}
            placeholderComponent={ChatPlaceholder}
            slashCommandConfig={slashCommandConfig}
            onUpdate={handleEditorUpdate}
            onSubmit={handleSubmit}
          />
        </div>

        <div className="flex shrink-0 items-center justify-between">
          {mcpIndicator ? (
            <McpIndicatorBadge indicator={mcpIndicator} />
          ) : (
            <div />
          )}
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
              disabled={disabled}
              className={cn([
                "inline-flex h-7 items-center gap-1.5 rounded-lg pr-1.5 pl-2.5 text-xs font-medium transition-all duration-100",
                "border",
                disabled
                  ? "cursor-default border-neutral-200 text-neutral-300"
                  : [
                      "border-stone-600 bg-stone-800 text-white",
                      "hover:bg-stone-700",
                      "active:scale-[0.97] active:bg-stone-600",
                    ],
                !hasContent && !disabled && "opacity-50",
              ])}
            >
              Send
              <span
                className={cn([
                  "font-mono text-xs",
                  disabled ? "text-neutral-300" : "text-stone-400",
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
  isRightPanel = false,
}: {
  children: React.ReactNode;
  hasContextBar?: boolean;
  isRightPanel?: boolean;
}) {
  return (
    <div
      className={cn(["relative min-h-0 shrink", !isRightPanel && "px-2 pb-2"])}
    >
      <div
        className={cn([
          "flex max-h-full flex-col border border-neutral-200 bg-white",
          isRightPanel ? "rounded-t-xl rounded-b-none" : "rounded-b-xl",
          hasContextBar && "rounded-t-none border-t-0",
        ])}
      >
        {children}
      </div>
    </div>
  );
}

const ChatPlaceholder: PlaceholderFunction = ({ node, pos }) => {
  "use no memo";
  if (node.type.name === "paragraph" && pos === 0) {
    return (
      <p className="text-sm text-neutral-400">
        Ask & search about anything, or be creative!
      </p>
    );
  }
  return "";
};
