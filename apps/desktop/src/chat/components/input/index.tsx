import "./chat-input.css";

import { useLingui } from "@lingui/react/macro";
import {
  ArrowUp,
  CircleNotch,
  Microphone,
  Square,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMemo, useRef } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { ChatEditor, type ChatEditorHandle } from "@anlg/editor/chat";
import type { PlaceholderFunction } from "@anlg/editor/plugins";
import { Button } from "@anlg/ui/components/ui/button";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

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
  const { elevatedSurfaceStyle } = useChatAppearance();
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
  const editorSemanticClassName = [
    "chat-input-editor",
    isFloating && hasVoiceStatus && "chat-input-editor-voice-active",
    isFloating &&
      hasContent &&
      !hasVoiceStatus &&
      "chat-input-editor-two-actions",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <Container
      elevatedSurfaceStyle={elevatedSurfaceStyle}
      isFloating={isFloating}
      isRightPanel={isRightPanel}
      hasVoiceStatus={hasVoiceStatus}
      indicator={
        history.position !== null && (
          <div
            data-chat-history-indicator
            {...stylex.props([
              styles.historyIndicator,
              isFloating
                ? styles.floatingHistoryIndicator
                : styles.elevatedHistoryIndicator,
            ])}
          >
            {t`History ${history.position}/${history.total}`}
          </div>
        )
      }
    >
      <div
        data-chat-message-input
        data-chat-layout={layout}
        data-chat-voice-state={dictation.phase}
        {...stylex.props([
          styles.messageInput,
          isFloating
            ? styles.floatingMessageInput
            : styles.elevatedMessageInput,
          isFloating &&
            (hasVoiceStatus
              ? styles.voiceMessageInput
              : styles.idleMessageInput),
        ])}
      >
        <div
          {...stylex.props(
            isFloating
              ? styles.floatingEditorContainer
              : styles.elevatedEditorContainer,
          )}
        >
          <ChatEditor
            ref={editorRef}
            className={
              mergeStyleXProps(
                [
                  styles.editor,
                  isFloating ? styles.floatingEditor : styles.elevatedEditor,
                  !isFloating &&
                    (isRightPanel
                      ? styles.rightPanelEditor
                      : styles.compactEditor),
                ],
                editorSemanticClassName,
              ).className
            }
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
            data-chat-input-actions
            data-chat-layout={layout}
            {...stylex.props([
              styles.actions,
              isFloating ? styles.floatingActions : styles.elevatedActions,
            ])}
          >
            {!isStreaming && (
              <button
                type="button"
                aria-label={t`Start voice input`}
                onClick={() => void dictation.start()}
                disabled={Boolean(disabled)}
                {...stylex.props(styles.voiceButton)}
              >
                <Microphone size={17} weight="regular" />
              </button>
            )}
            {isStreaming ? (
              <Button
                onClick={onStop}
                size="icon"
                variant="ghost"
                sx={styles.roundIconButton}
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
  elevatedSurfaceStyle,
  isFloating,
  isRightPanel,
  hasVoiceStatus,
  indicator,
}: {
  children: React.ReactNode;
  elevatedSurfaceStyle: StyleXProps["sx"];
  isFloating: boolean;
  isRightPanel: boolean;
  hasVoiceStatus: boolean;
  indicator?: React.ReactNode;
}) {
  return (
    <div
      data-chat-input-container
      data-chat-layout={isRightPanel ? "right-panel" : "floating"}
      {...stylex.props([
        styles.container,
        isRightPanel ? styles.rightPanelContainer : styles.floatingContainer,
      ])}
    >
      {indicator}
      <div
        data-chat-input-surface={isFloating ? "floating" : "elevated"}
        {...stylex.props([
          styles.inputSurface,
          isFloating
            ? styles.floatingInputSurface
            : [styles.elevatedInputSurface, elevatedSurfaceStyle],
          isFloating &&
            (hasVoiceStatus
              ? styles.voiceInputSurface
              : styles.idleInputSurface),
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
      {...stylex.props([
        styles.sendButton,
        !disabled && styles.enabledSendButton,
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
    <div {...stylex.props(styles.voiceStatus)}>
      <div aria-hidden="true" {...stylex.props(styles.waveform)}>
        {isProcessing ? (
          <div {...stylex.props(styles.processingStatus)}>
            <CircleNotch {...stylex.props(styles.smallSpinner)} />
            <span>
              {phase === "starting" ? t`Starting…` : t`Transcribing…`}
            </span>
          </div>
        ) : (
          WAVEFORM_HEIGHTS.map((height, index) => (
            <span
              key={index}
              {...mergeStyleXProps(styles.waveformBar, undefined, {
                height,
                animationDelay: `${index * -70}ms`,
              })}
            />
          ))
        )}
      </div>
      {!isProcessing && (
        <span {...stylex.props(styles.elapsedTime)}>
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
        {...stylex.props(styles.stopVoiceButton)}
      >
        {isProcessing ? (
          <CircleNotch {...stylex.props(styles.smallSpinner)} />
        ) : (
          <Square size={12} weight="fill" />
        )}
      </button>
      {isStreaming ? (
        <Button
          onClick={onStopResponse}
          size="icon"
          variant="ghost"
          sx={styles.roundIconButton}
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

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const waveform = stylex.keyframes({
  "0%, 100%": {
    transform: "scaleY(0.45)",
  },
  "50%": {
    transform: "scaleY(1)",
  },
});

const styles = stylex.create({
  historyIndicator: {
    color: `color-mix(in oklab, ${colors.mutedForeground} 80%, transparent)`,
    fontSize: "0.6875rem",
    lineHeight: 1,
    paddingBottom: "0.25rem",
  },
  floatingHistoryIndicator: {
    paddingInline: "1rem",
  },
  elevatedHistoryIndicator: {
    paddingInline: "0.5rem",
  },
  messageInput: {
    display: "flex",
  },
  floatingMessageInput: {
    maxHeight: "100%",
    minHeight: "30px",
    minWidth: 0,
    position: "relative",
    width: "100%",
  },
  elevatedMessageInput: {
    flexDirection: "column",
    paddingBottom: "0.5rem",
    paddingInline: "0.5rem",
    paddingTop: "0.75rem",
  },
  voiceMessageInput: {
    alignItems: "stretch",
    flexDirection: "column",
  },
  idleMessageInput: {
    alignItems: "center",
  },
  floatingEditorContainer: {
    flex: "1",
    minWidth: 0,
  },
  elevatedEditorContainer: {
    marginBottom: "0.25rem",
    minHeight: 0,
  },
  editor: {
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    overflowY: "auto",
    overscrollBehavior: "contain",
  },
  floatingEditor: {
    maxHeight: "9rem",
    minHeight: "1.25rem",
    minWidth: 0,
    width: "100%",
  },
  elevatedEditor: {
    maxHeight: "12rem",
  },
  rightPanelEditor: {
    maxHeight: "40vh",
  },
  compactEditor: {
    maxHeight: "12rem",
  },
  actions: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    gap: "0.25rem",
  },
  floatingActions: {
    bottom: "0.125rem",
    position: "absolute",
    right: 0,
  },
  elevatedActions: {
    justifyContent: "flex-end",
  },
  voiceButton: {
    alignItems: "center",
    backgroundColor: {
      default: "transparent",
      ":hover": colors.muted,
    },
    borderRadius: radii.full,
    color: colors.mutedForeground,
    cursor: {
      default: "pointer",
      ":disabled": "default",
    },
    display: "inline-flex",
    flexShrink: 0,
    height: "1.75rem",
    justifyContent: "center",
    opacity: {
      default: 1,
      ":disabled": 0.45,
    },
    transitionDuration: "150ms",
    transitionProperty: "color, background-color, border-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "1.75rem",
  },
  roundIconButton: {
    borderRadius: radii.full,
    height: "1.75rem",
    width: "1.75rem",
  },
  container: {
    flexShrink: 0,
    minWidth: 0,
    position: "relative",
  },
  floatingContainer: {
    paddingBottom: "0.25rem",
    paddingInline: "0.25rem",
  },
  rightPanelContainer: {
    paddingBottom: "0.75rem",
    paddingInline: "0.5rem",
  },
  inputSurface: {
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    maxHeight: "100%",
  },
  floatingInputSurface: {
    backgroundColor: {
      default: "#ffffff",
      ":is(.dark *)": colors.card,
    },
    borderColor: `color-mix(in oklab, ${colors.border} 70%, transparent)`,
    borderRadius: "19px",
    boxShadow: "none",
    color: colors.cardForeground,
    flexDirection: "row",
    fontSize: "0.875rem",
    maxHeight: "10rem",
    minHeight: "38px",
    overflow: "hidden",
    paddingLeft: "1rem",
    paddingRight: "6px",
  },
  elevatedInputSurface: {
    borderRadius: radii.xl,
    flexDirection: "column",
  },
  voiceInputSurface: {
    alignItems: "stretch",
    paddingBlock: "0.5rem",
  },
  idleInputSurface: {
    alignItems: "center",
    paddingBlock: "3px",
  },
  sendButton: {
    alignItems: "center",
    borderColor: colors.border,
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    color: `color-mix(in oklab, ${colors.mutedForeground} 60%, transparent)`,
    cursor: {
      default: "pointer",
      ":disabled": "default",
    },
    display: "inline-flex",
    flexShrink: 0,
    height: "1.75rem",
    justifyContent: "center",
    transitionDuration: "100ms",
    transitionProperty: "all",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "1.75rem",
  },
  enabledSendButton: {
    backgroundColor: {
      default: colors.primary,
      ":active": `color-mix(in oklab, ${colors.primary} 80%, transparent)`,
      ":hover": `color-mix(in oklab, ${colors.primary} 90%, transparent)`,
    },
    borderColor: "oklch(44.4% 0.011 73.639)",
    color: colors.primaryForeground,
    transform: {
      default: "scale(1)",
      ":active": "scale(0.97)",
    },
  },
  voiceStatus: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    marginTop: "0.5rem",
    minHeight: "1.75rem",
    width: "100%",
  },
  waveform: {
    alignItems: "center",
    display: "flex",
    flex: "1",
    gap: "3px",
    minWidth: 0,
    overflow: "hidden",
  },
  processingStatus: {
    alignItems: "center",
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.75rem",
    gap: "0.5rem",
    lineHeight: "1rem",
  },
  smallSpinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    height: "0.875rem",
    width: "0.875rem",
  },
  waveformBar: {
    animationDuration: "700ms",
    animationIterationCount: "infinite",
    animationName: waveform,
    animationTimingFunction: "ease-in-out",
    backgroundColor: `color-mix(in oklab, ${colors.mutedForeground} 55%, transparent)`,
    borderRadius: radii.full,
    flexShrink: 0,
    transformOrigin: "center",
    width: "1px",
  },
  elapsedTime: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    fontVariantNumeric: "tabular-nums",
    lineHeight: "1rem",
  },
  stopVoiceButton: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderRadius: radii.full,
    color: colors.foreground,
    display: "inline-flex",
    flexShrink: 0,
    height: "1.75rem",
    justifyContent: "center",
    opacity: {
      default: 1,
      ":disabled": 0.6,
    },
    width: "1.75rem",
  },
});

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
