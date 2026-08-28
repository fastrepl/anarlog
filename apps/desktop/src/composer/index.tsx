import { t } from "@lingui/core/macro";
import {
  ArrowUp,
  ArrowUpRight,
  GearSix,
  Sparkle,
  X,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { platform } from "@tauri-apps/plugin-os";
import { useEffect, useRef } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
import { ChatEditor, type ChatEditorHandle } from "@anlg/editor/chat";
import type { PlaceholderFunction } from "@anlg/editor/plugins";
import { commands as windowsCommands } from "@anlg/plugin-windows";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { useLanguageModel } from "~/ai/hooks";
import {
  useAutoFocusEditor,
  useDraftState,
  useSubmit,
} from "~/chat/components/input/hooks";
import { ChatSession } from "~/chat/components/session-provider";
import { dedupeByKey, type ContextRef } from "~/chat/context/entities";
import { useChatGroup } from "~/chat/store/queries";
import { useChatActions } from "~/chat/store/use-chat-actions";
import { useShell } from "~/contexts/shell";
import { useMentionConfig } from "~/editor-bridge/mention-config";
import { useOwnerUserId } from "~/shared/owner-user";

export function ComposerScreen() {
  const { chat } = useShell();
  const model = useLanguageModel("chat");
  const userId = useOwnerUserId();
  const currentChatGroup = useChatGroup(chat.groupId, chat.scope);
  const { handleSendMessage } = useChatActions({
    chatScope: chat.scope,
    groupId: chat.groupId,
    onGroupCreated: chat.setGroupId,
    onGroupCreateFailed: chat.rollbackFailedGroup,
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

  if (!userId) {
    return <div {...stylex.props(styles.screen)} />;
  }

  return (
    <div {...stylex.props(styles.screen)}>
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
              title={currentChatGroup?.title || t`Ask Anarlog AI anything`}
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
    <div {...stylex.props(styles.card)}>
      <div {...stylex.props(styles.settingsHeader)}>
        <div data-tauri-drag-region {...stylex.props(styles.headerCopy)}>
          <p {...stylex.props(styles.eyebrow)}>{t`Composer`}</p>
          <p {...stylex.props(styles.settingsSubtitle)}>
            {t`Configure a chat model to use the quick composer.`}
          </p>
        </div>

        <button
          type="button"
          onClick={() => void dismissComposer()}
          data-tauri-drag-region="false"
          {...stylex.props(styles.closeButton)}
        >
          <X {...stylex.props(styles.iconMd)} />
        </button>
      </div>

      <button
        type="button"
        onClick={() => void openSettingsInMainWindow()}
        {...stylex.props(styles.settingsButton)}
      >
        <GearSix {...stylex.props(styles.iconMd)} />
        {t`Configure a chat model in Settings`}
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
  const primaryModifier = platform() === "macos" ? "⌘" : "Ctrl";

  useAutoFocusEditor({
    editorRef,
    disabled,
  });

  return (
    <div {...stylex.props(styles.card)}>
      <div {...stylex.props(styles.composerHeader)}>
        <div data-tauri-drag-region {...stylex.props(styles.headerCopy)}>
          <p {...stylex.props(styles.eyebrow)}>{t`Composer`}</p>
          <p {...stylex.props(styles.composerTitle)}>{title}</p>
        </div>

        <div
          data-tauri-drag-region="false"
          {...stylex.props(styles.headerActions)}
        >
          <button
            type="button"
            onClick={() => void openMainWindow()}
            data-tauri-drag-region="false"
            {...stylex.props(styles.openButton)}
          >
            <ArrowUpRight {...stylex.props(styles.iconSm)} />
            {t`Open Anarlog`}
          </button>
          <button
            type="button"
            onClick={() => void dismissComposer()}
            data-tauri-drag-region="false"
            {...stylex.props(styles.closeButton)}
          >
            <X {...stylex.props(styles.iconMd)} />
          </button>
        </div>
      </div>

      <ChatEditor
        ref={editorRef}
        onAttachmentError={(message) => sonnerToast.error(message)}
        sx={styles.editor}
        initialContent={initialContent}
        mentionConfig={mentionConfig}
        placeholder={composerPlaceholder}
        onUpdate={handleEditorUpdate}
        onSubmit={handleSubmit}
      />

      <div {...stylex.props(styles.footer)}>
        <div {...stylex.props(styles.hints)}>
          <span {...stylex.props(styles.hint)}>{t`Esc to dismiss`}</span>
          <span {...stylex.props(styles.hint)}>
            {t`${primaryModifier} ↩ to send`}
          </span>
        </div>

        {isStreaming ? (
          <button
            type="button"
            onClick={onStop}
            {...stylex.props(styles.stopButton)}
          >
            <Sparkle {...stylex.props(styles.iconSm)} />
            {t`Stop`}
          </button>
        ) : (
          <button
            type="button"
            onClick={handleSubmit}
            disabled={disabled}
            {...stylex.props(
              styles.sendButton,
              disabled ? styles.sendButtonDisabled : styles.sendButtonEnabled,
              !hasContent && !disabled && styles.sendButtonEmpty,
            )}
          >
            <ArrowUp {...stylex.props(styles.iconMd)} />
          </button>
        )}
      </div>
    </div>
  );
}

const composerPlaceholder: PlaceholderFunction = ({ node, pos }) => {
  if (node.type.name === "paragraph" && pos === 0) {
    return t`Message Anarlog AI`;
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

const styles = stylex.create({
  card: {
    backgroundColor: `color-mix(in oklab, ${colors.primary} 88%, transparent)`,
    borderRadius: "28px",
    color: colors.primaryForeground,
    height: "100%",
    paddingBlock: "1rem",
    paddingInline: "1.25rem",
    width: "100%",
  },
  closeButton: {
    alignItems: "center",
    backgroundColor: {
      default: `color-mix(in oklab, ${colors.primaryForeground} 7%, transparent)`,
      ":hover": `color-mix(in oklab, ${colors.primaryForeground} 12%, transparent)`,
    },
    borderRadius: radii.full,
    color: {
      default: `color-mix(in oklab, ${colors.primaryForeground} 65%, transparent)`,
      ":hover": colors.primaryForeground,
    },
    display: "inline-flex",
    height: "2rem",
    justifyContent: "center",
    transitionDuration: "150ms",
    transitionProperty:
      "color, background-color, border-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "2rem",
  },
  composerHeader: {
    alignItems: "flex-start",
    display: "flex",
    gap: "1rem",
    justifyContent: "space-between",
    marginBottom: "0.75rem",
  },
  composerTitle: {
    color: `color-mix(in oklab, ${colors.primaryForeground} 90%, transparent)`,
    fontSize: "15px",
    overflow: "hidden",
    paddingTop: "0.25rem",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  editor: {
    color: {
      default: colors.primaryForeground,
      "::placeholder": `color-mix(in oklab, ${colors.primaryForeground} 28%, transparent)`,
      ":is(*) .ProseMirror::placeholder": `color-mix(in oklab, ${colors.primaryForeground} 28%, transparent)`,
    },
    fontSize: "15px",
    lineHeight: "1.5rem",
    maxHeight: "88px",
    minHeight: {
      default: "34px",
      ":is(*) .ProseMirror": "34px",
    },
    outline: {
      default: null,
      ":is(*) .ProseMirror": "none",
    },
    overflowY: "auto",
  },
  eyebrow: {
    color: `color-mix(in oklab, ${colors.primaryForeground} 38%, transparent)`,
    fontSize: "10px",
    fontWeight: 600,
    letterSpacing: "0.24em",
    textTransform: "uppercase",
  },
  footer: {
    alignItems: "center",
    display: "flex",
    gap: "0.75rem",
    justifyContent: "space-between",
    marginTop: "0.75rem",
  },
  headerActions: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
  },
  headerCopy: {
    flex: "1",
    minWidth: 0,
    paddingRight: "1rem",
  },
  hint: {
    backgroundColor: `color-mix(in oklab, ${colors.primaryForeground} 8%, transparent)`,
    borderRadius: radii.full,
    paddingBlock: "0.25rem",
    paddingInline: "0.5rem",
  },
  hints: {
    alignItems: "center",
    color: `color-mix(in oklab, ${colors.primaryForeground} 40%, transparent)`,
    display: "flex",
    fontSize: "11px",
    gap: "0.5rem",
  },
  iconMd: {
    height: "1rem",
    width: "1rem",
  },
  iconSm: {
    height: "0.875rem",
    width: "0.875rem",
  },
  openButton: {
    alignItems: "center",
    backgroundColor: {
      default: `color-mix(in oklab, ${colors.primaryForeground} 7%, transparent)`,
      ":hover": `color-mix(in oklab, ${colors.primaryForeground} 12%, transparent)`,
    },
    borderRadius: radii.full,
    color: {
      default: `color-mix(in oklab, ${colors.primaryForeground} 76%, transparent)`,
      ":hover": colors.primaryForeground,
    },
    display: "inline-flex",
    fontSize: "0.75rem",
    fontWeight: 500,
    gap: "0.375rem",
    paddingBlock: "0.375rem",
    paddingInline: "0.75rem",
    transitionDuration: "150ms",
    transitionProperty:
      "color, background-color, border-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  screen: {
    backgroundColor: "transparent",
    height: "100vh",
    width: "100vw",
  },
  sendButton: {
    alignItems: "center",
    borderRadius: radii.full,
    display: "inline-flex",
    height: "2.5rem",
    justifyContent: "center",
    width: "2.5rem",
  },
  sendButtonDisabled: {
    backgroundColor: `color-mix(in oklab, ${colors.primaryForeground} 8%, transparent)`,
    color: `color-mix(in oklab, ${colors.primaryForeground} 25%, transparent)`,
    cursor: "default",
  },
  sendButtonEmpty: {
    opacity: 0.55,
  },
  sendButtonEnabled: {
    backgroundColor: colors.primaryForeground,
    color: colors.primary,
    transform: {
      default: null,
      ":hover": "scale(1.02)",
    },
    transitionDuration: "150ms",
    transitionProperty: "transform",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  settingsButton: {
    alignItems: "center",
    backgroundColor: {
      default: `color-mix(in oklab, ${colors.primaryForeground} 7%, transparent)`,
      ":hover": `color-mix(in oklab, ${colors.primaryForeground} 10%, transparent)`,
    },
    borderRadius: radii.full,
    color: {
      default: `color-mix(in oklab, ${colors.primaryForeground} 85%, transparent)`,
      ":hover": colors.primaryForeground,
    },
    display: "inline-flex",
    fontSize: "0.875rem",
    fontWeight: 500,
    gap: "0.5rem",
    paddingBlock: "0.5rem",
    paddingInline: "0.875rem",
    transitionDuration: "150ms",
    transitionProperty:
      "color, background-color, border-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
  settingsHeader: {
    alignItems: "center",
    display: "flex",
    gap: "0.75rem",
    justifyContent: "space-between",
    marginBottom: "0.75rem",
  },
  settingsSubtitle: {
    color: `color-mix(in oklab, ${colors.primaryForeground} 72%, transparent)`,
    fontSize: "0.875rem",
    overflow: "hidden",
    paddingTop: "0.25rem",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  stopButton: {
    alignItems: "center",
    backgroundColor: {
      default: `color-mix(in oklab, ${colors.primaryForeground} 8%, transparent)`,
      ":hover": `color-mix(in oklab, ${colors.primaryForeground} 12%, transparent)`,
    },
    borderRadius: radii.full,
    color: {
      default: `color-mix(in oklab, ${colors.primaryForeground} 82%, transparent)`,
      ":hover": colors.primaryForeground,
    },
    display: "inline-flex",
    fontSize: "0.75rem",
    fontWeight: 500,
    gap: "0.5rem",
    paddingBlock: "0.375rem",
    paddingInline: "0.75rem",
    transitionDuration: "150ms",
    transitionProperty:
      "color, background-color, border-color, text-decoration-color, fill, stroke",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
});
