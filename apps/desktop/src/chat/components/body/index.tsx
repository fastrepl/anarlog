import { Trans } from "@lingui/react/macro";
import { CaretDown } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import type { ChatStatus } from "ai";

import { Button } from "@anlg/ui/components/ui/button";

import { ChatBodyEmpty } from "./empty";
import { ChatBodyNonEmpty } from "./non-empty";
import { useChatAutoScroll } from "./use-chat-auto-scroll";

import type { ContextRef } from "~/chat/context/entities";
import { chatFloatingControlStyle } from "~/chat/surface";
import type { AnlgUIMessage } from "~/chat/types";
import { useShell } from "~/contexts/shell";

export function ChatBody({
  messages,
  status,
  error,
  onReload,
  isModelConfigured = true,
  hasContext = false,
  onSendMessage,
}: {
  messages: AnlgUIMessage[];
  status: ChatStatus;
  error?: Error;
  onReload?: () => void;
  isModelConfigured?: boolean;
  hasContext?: boolean;
  onSendMessage?: (
    content: string,
    parts: Array<{ type: "text"; text: string }>,
    contextRefs?: ContextRef[],
  ) => void;
}) {
  const { chat } = useShell();
  const isRightPanel = chat.mode === "RightPanelOpen";
  const isFloating = chat.mode === "FloatingOpen";
  const {
    contentRef,
    isAtBottom,
    scrollRef,
    scrollToBottom,
    showGoToRecent,
    updateAutoScrollState,
    handleKeyDown,
    handlePointerDown,
    handlePointerMove,
    handleWheel,
  } = useChatAutoScroll(status);

  return (
    <div
      data-chat-body-layout={isRightPanel ? "right-panel" : "floating"}
      {...stylex.props([
        styles.root,
        isRightPanel ? styles.rightPanelRoot : styles.floatingRoot,
      ])}
    >
      <div
        data-chat-scroll-area
        ref={scrollRef}
        onKeyDown={handleKeyDown}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onScroll={updateAutoScrollState}
        onWheel={handleWheel}
        {...stylex.props([
          styles.scrollArea,
          isRightPanel
            ? styles.rightPanelScrollArea
            : styles.floatingScrollArea,
        ])}
      >
        <div
          data-chat-body-content
          ref={contentRef}
          {...stylex.props([
            styles.content,
            isRightPanel ? styles.rightPanelContent : styles.floatingContent,
          ])}
        >
          {!isFloating && <div {...stylex.props(styles.spacer)} />}
          {messages.length === 0 ? (
            <ChatBodyEmpty
              isModelConfigured={isModelConfigured}
              hasContext={hasContext}
              onSendMessage={onSendMessage}
            />
          ) : (
            <ChatBodyNonEmpty
              messages={messages}
              status={status}
              error={error}
              onReload={onReload}
            />
          )}
        </div>
      </div>
      {messages.length > 0 && showGoToRecent && !isAtBottom && (
        <Button
          onClick={scrollToBottom}
          size="sm"
          sx={[styles.goToRecent, chatFloatingControlStyle]}
          variant="outline"
        >
          <CaretDown size={12} />
          <span {...stylex.props(styles.goToRecentText)}>
            <Trans>Go to recent</Trans>
          </span>
        </Button>
      )}
    </div>
  );
}

const styles = stylex.create({
  root: {
    display: "flex",
    flexDirection: "column",
    isolation: "isolate",
    minHeight: 0,
    position: "relative",
  },
  floatingRoot: {
    flexGrow: "1",
    flexShrink: "1",
    flexBasis: "auto",
  },
  rightPanelRoot: {
    flex: "1",
  },
  scrollArea: {
    display: "flex",
    flexDirection: "column",
    minHeight: 0,
    overflowY: "auto",
  },
  floatingScrollArea: {
    flexGrow: "1",
    flexShrink: "1",
    flexBasis: "auto",
    maxHeight: "min(36rem, 70vh)",
  },
  rightPanelScrollArea: {
    flex: "1",
  },
  content: {
    display: "flex",
    flexDirection: "column",
    paddingInline: "0.75rem",
  },
  floatingContent: {
    paddingBlock: "0.75rem",
  },
  rightPanelContent: {
    flex: "1",
    minHeight: "100%",
    paddingBlock: "1.25rem",
  },
  spacer: {
    flex: "1",
  },
  goToRecent: {
    alignItems: "center",
    borderStyle: "solid",
    borderWidth: "1px",
    borderRadius: "9999px",
    bottom: "0.75rem",
    boxShadow: "0 1px 2px 0 rgb(0 0 0 / 0.05)",
    display: "flex",
    gap: "0.25rem",
    left: "50%",
    position: "absolute",
    transform: "translateX(-50%)",
    zIndex: 1,
  },
  goToRecentText: {
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
});
