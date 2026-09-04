import "./chat-input.css";

import { useLingui } from "@lingui/react/macro";
import { useMemo, useRef } from "react";

import { ChatEditor, type ChatEditorHandle } from "@anlg/editor/chat";
import type { PlaceholderFunction } from "@anlg/editor/plugins";
import {
  ArrowUp,
  CircleNotch,
  Microphone,
  Square,
} from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { cn } from "@anlg/utils";

import {
  useAutoFocusEditor,
  useDraftState,
  useMessageHistory,
  useSubmit,
} from "./hooks";
import { useDictation } from "./use-dictation";

import type { ContextRef } from "~/chat/context/entities";
import { useChatAppearance } from "~/chat/hooks/use-chat-appearance";
import { useShell } from "~/contexts/shell";
import { useMentionConfig } from "~/editor-bridge/mention-config";

export function ChatMessageInput({
  draftKey,
  layout = "floating",
  onSendMessage,
  disabled: disabledProp,
  isStreaming,
  onStop,
  onDraftContentChange,
  onContextRefsChange,
}: {
  draftKey: string;
  layout?: "floating" | "right-panel";
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

  const history = useMessageHistory({ editorRef });
  const { hasContent, initialContent, handleEditorUpdate } = useDraftState({
    draftKey,
    onDraftContentChange,
    onContextRefsChange,
    onUserEdit: history.handleUserEdit,
    shouldPersistUpdate: history.shouldPersistUpdate,
  });
  const handleSubmit = useSubmit({
    draftKey,
    editorRef,
    disabled,
    onSendMessage,
    onDraftContentChange,
    onContextRefsChange,
    onSubmitted: history.handleSubmitted,
  });
  const dictation = useDictation({
    editorRef,
    disabled: Boolean(disabled) || Boolean(isStreaming),
  });
  useAutoFocusEditor({ editorRef, disabled, shouldFocus });
  const mentionConfig = useMentionConfig();
  const isSendDisabled = Boolean(disabled) || !hasContent;
  const isRightPanel = layout === "right-panel";
  const isFloating = layout === "floating";
  const showSendControl = !isFloating || isStreaming || hasContent;
  const hasVoiceStatus = dictation.phase !== "idle";
  const placeholderText = t`Ask anything`;
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
      hasVoiceStatus={hasVoiceStatus}
      indicator={
        history.position !== null && (
          <div
            data-chat-history-indicator
            className={cn([
              "text-muted-foreground/80 pb-1 text-[11px] leading-none",
              isFloating ? "px-4" : "px-2",
            ])}
          >
            {t`History ${history.position}/${history.total}`}
          </div>
        )
      }
    >
      <div
        data-chat-message-input
        className={cn([
          isFloating
            ? [
                "relative flex max-h-full min-h-[30px] w-full min-w-0",
                hasVoiceStatus ? "flex-col items-stretch" : "items-center",
              ]
            : "flex flex-col px-2 pt-3 pb-2",
        ])}
        data-chat-voice-state={dictation.phase}
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
              isFloating && hasVoiceStatus && "chat-input-editor-voice-active",
              isFloating &&
                hasContent &&
                !hasVoiceStatus &&
                "chat-input-editor-two-actions",
            ])}
            initialContent={initialContent}
            mentionConfig={mentionConfig}
            placeholder={placeholder}
            submitShortcut="enter"
            onAttachmentError={(message) => sonnerToast.error(message)}
            onUpdate={handleEditorUpdate}
            onSubmit={handleSubmit}
            onHistoryNavigate={history.navigate}
          />
        </div>

        {dictation.phase !== "idle" ? (
          <VoiceStatus
            elapsedSeconds={dictation.elapsedSeconds}
            isSendDisabled={isSendDisabled}
            isStreaming={Boolean(isStreaming)}
            onSend={handleSubmit}
            onStop={() => void dictation.stop()}
            onStopResponse={onStop}
            phase={dictation.phase}
            showSend={showSendControl && !isStreaming}
          />
        ) : (
          <div
            className={cn([
              "flex shrink-0 items-center gap-1",
              isFloating ? "absolute right-0 bottom-0.5" : "justify-end",
            ])}
          >
            {!isStreaming && (
              <button
                type="button"
                aria-label={t`Start voice input`}
                onClick={() => void dictation.start()}
                disabled={Boolean(disabled)}
                className={cn([
                  "text-muted-foreground hover:bg-muted inline-flex size-7 shrink-0 items-center justify-center rounded-full transition-colors",
                  "disabled:cursor-default disabled:opacity-45",
                ])}
              >
                <Microphone size={17} weight="regular" />
              </button>
            )}
            {isStreaming ? (
              <Button
                onClick={onStop}
                size="icon"
                variant="ghost"
                className="h-7 w-7 rounded-full"
                aria-label={t`Stop response`}
              >
                <Square size={14} weight="fill" />
              </Button>
            ) : showSendControl ? (
              <SendButton disabled={isSendDisabled} onClick={handleSubmit} />
            ) : null}
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
  hasVoiceStatus,
  indicator,
}: {
  children: React.ReactNode;
  elevatedSurfaceClassName: string;
  isFloating: boolean;
  isRightPanel: boolean;
  hasVoiceStatus: boolean;
  indicator?: React.ReactNode;
}) {
  return (
    <div
      className={cn([
        "relative min-w-0 shrink-0",
        isRightPanel ? "px-2 pb-3" : "px-1 pb-1",
      ])}
    >
      {indicator}
      <div
        data-chat-input-surface={isFloating ? "floating" : "elevated"}
        className={cn([
          "flex max-h-full border",
          isFloating
            ? [
                "border-border/70 text-card-foreground max-h-40 min-h-[38px] flex-row overflow-hidden rounded-[19px] bg-white pr-[6px] pl-4 text-sm shadow-none",
                "dark:bg-card dark:text-card-foreground",
                hasVoiceStatus ? "items-stretch py-2" : "items-center py-[3px]",
              ]
            : [elevatedSurfaceClassName, "flex-col rounded-xl"],
        ])}
      >
        {children}
      </div>
    </div>
  );
}

function SendButton({
  disabled,
  onClick,
}: {
  disabled: boolean;
  onClick: () => void;
}) {
  const { t } = useLingui();

  return (
    <button
      type="button"
      aria-label={t`Send message`}
      onClick={onClick}
      disabled={disabled}
      className={cn([
        "chat-input-send",
        "border-border text-muted-foreground/60 inline-flex size-7 shrink-0 items-center justify-center rounded-full border transition-all duration-100",
        !disabled && [
          "bg-primary text-primary-foreground border-stone-600",
          "hover:bg-primary/90",
          "active:bg-primary/80 active:scale-[0.97]",
        ],
      ])}
    >
      <ArrowUp size={15} weight="bold" />
    </button>
  );
}

const WAVEFORM_HEIGHTS = [3, 7, 5, 10, 6, 12, 8, 4, 9, 6, 11, 5, 8, 3];

function VoiceStatus({
  elapsedSeconds,
  isSendDisabled,
  isStreaming,
  onSend,
  onStop,
  onStopResponse,
  phase,
  showSend,
}: {
  elapsedSeconds: number;
  isSendDisabled: boolean;
  isStreaming: boolean;
  onSend: () => void;
  onStop: () => void;
  onStopResponse?: () => void;
  phase: "starting" | "recording" | "transcribing";
  showSend: boolean;
}) {
  const { t } = useLingui();
  const isProcessing = phase !== "recording";

  return (
    <div className="mt-2 flex min-h-7 w-full items-center gap-2">
      <div
        aria-hidden="true"
        className="flex min-w-0 flex-1 items-center gap-[3px] overflow-hidden"
      >
        {isProcessing ? (
          <div className="text-muted-foreground flex items-center gap-2 text-xs">
            <CircleNotch className="size-3.5 animate-spin" />
            <span>
              {phase === "starting" ? t`Starting…` : t`Transcribing…`}
            </span>
          </div>
        ) : (
          WAVEFORM_HEIGHTS.map((height, index) => (
            <span
              key={index}
              className="chat-input-waveform-bar bg-muted-foreground/55 w-px shrink-0 rounded-full"
              style={{
                height,
                animationDelay: `${index * -70}ms`,
              }}
            />
          ))
        )}
      </div>
      {!isProcessing && (
        <span className="text-muted-foreground text-xs tabular-nums">
          {formatElapsedTime(elapsedSeconds)}
        </span>
      )}
      <button
        type="button"
        aria-label={
          phase === "starting"
            ? t`Starting voice input`
            : phase === "transcribing"
              ? t`Transcribing voice input`
              : t`Stop voice input`
        }
        onClick={onStop}
        disabled={isProcessing}
        className="bg-muted text-foreground inline-flex size-7 shrink-0 items-center justify-center rounded-full disabled:opacity-60"
      >
        {isProcessing ? (
          <CircleNotch className="size-3.5 animate-spin" />
        ) : (
          <Square size={12} weight="fill" />
        )}
      </button>
      {isStreaming ? (
        <Button
          onClick={onStopResponse}
          size="icon"
          variant="ghost"
          className="h-7 w-7 rounded-full"
          aria-label={t`Stop response`}
        >
          <Square size={14} weight="fill" />
        </Button>
      ) : (
        showSend && <SendButton disabled={isSendDisabled} onClick={onSend} />
      )}
    </div>
  );
}

function formatElapsedTime(elapsedSeconds: number) {
  const minutes = Math.floor(elapsedSeconds / 60);
  const seconds = elapsedSeconds % 60;
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
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
