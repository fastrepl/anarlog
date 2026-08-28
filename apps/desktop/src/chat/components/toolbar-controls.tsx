import { Trans, useLingui } from "@lingui/react/macro";
import {
  CaretDown,
  ChatCircle,
  ClockCounterClockwise,
  PictureInPicture,
  Plus,
  SidebarSimple,
  X,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useState } from "react";

import { colors, radii, shadows } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import type { StyleXProps } from "@anlg/ui/lib/stylex";
import { formatDistanceToNow } from "@anlg/utils";

import {
  type ChatGroupRecord,
  useRecentChatGroups,
} from "~/chat/store/queries";
import type { ChatScope } from "~/chat/types";

export function ChatToolbarControls({
  chatScope,
  currentChatGroupId,
  layout = "floating",
  onClose,
  onNewChat,
  onOpenFloating,
  onOpenRightPanel,
  onSelectChat,
  surface = "light",
}: {
  chatScope: ChatScope;
  currentChatGroupId: string | undefined;
  layout?: "floating" | "right-panel";
  onClose?: () => void;
  onNewChat: () => void;
  onOpenFloating?: () => void;
  onOpenRightPanel?: () => void;
  onSelectChat: (chatGroupId: string) => void;
  surface?: "light" | "dark";
}) {
  const { t } = useLingui();
  const isRightPanel = layout === "right-panel";
  const actionButtonStyle = [
    toolbarButtonStyles[surface],
    isRightPanel && styles.compactActionButton,
  ];

  return (
    <div
      data-tauri-drag-region={isRightPanel || undefined}
      data-chat-toolbar-layout={layout}
      {...stylex.props([
        styles.toolbar,
        isRightPanel ? styles.rightPanelToolbar : styles.floatingToolbar,
      ])}
    >
      <div
        data-tauri-drag-region={isRightPanel || undefined}
        {...stylex.props(styles.historySlot)}
      >
        <ChatGroups
          chatScope={chatScope}
          currentChatGroupId={currentChatGroupId}
          layout={layout}
          onSelectChat={onSelectChat}
          surface={surface}
        />
      </div>
      <div
        data-tauri-drag-region={isRightPanel || undefined}
        data-chat-toolbar-actions
        {...stylex.props(styles.actions)}
      >
        <ChatActionButton
          icon={<Plus size={16} />}
          label={t`New chat`}
          onClick={onNewChat}
          sx={actionButtonStyle}
        />
        {isRightPanel ? (
          <>
            <ChatActionButton
              icon={<PictureInPicture size={16} />}
              label={t`Float chat`}
              onClick={onOpenFloating ?? (() => {})}
              sx={actionButtonStyle}
            />
            <ChatActionButton
              icon={<X size={16} />}
              label={t`Close chat`}
              onClick={onClose ?? (() => {})}
              sx={actionButtonStyle}
            />
          </>
        ) : (
          <>
            <ChatActionButton
              icon={<SidebarSimple size={16} />}
              label={t`Open in right panel`}
              onClick={onOpenRightPanel ?? (() => {})}
              sx={actionButtonStyle}
            />
          </>
        )}
      </div>
    </div>
  );
}

function ChatActionButton({
  icon,
  label,
  onClick,
  sx,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  sx?: StyleXProps["sx"];
}) {
  return (
    <Button
      aria-label={label}
      data-tauri-drag-region="false"
      onClick={onClick}
      size="icon"
      variant="ghost"
      sx={[styles.actionButton, sx]}
    >
      {icon}
    </Button>
  );
}

function ChatGroups({
  chatScope,
  currentChatGroupId,
  layout,
  onSelectChat,
  surface = "light",
}: {
  chatScope: ChatScope;
  currentChatGroupId: string | undefined;
  layout: "floating" | "right-panel";
  onSelectChat: (chatGroupId: string) => void;
  surface?: "light" | "dark";
}) {
  const { t } = useLingui();
  const [isDropdownOpen, setIsDropdownOpen] = useState(false);
  const isDark = surface === "dark";

  const recentChatGroups = useRecentChatGroups(chatScope, 5);

  return (
    <DropdownMenu open={isDropdownOpen} onOpenChange={setIsDropdownOpen}>
      <DropdownMenuTrigger asChild>
        <Button
          aria-label={t`Chat history`}
          data-tauri-drag-region="false"
          variant="ghost"
          size="sm"
          sx={[
            styles.historyTrigger,
            layout === "right-panel" && styles.compactHistoryTrigger,
            isDark ? styles.darkHistoryTrigger : styles.lightHistoryTrigger,
          ]}
        >
          <ClockCounterClockwise
            {...stylex.props([
              styles.historyIcon,
              isDark ? styles.darkHistoryIcon : styles.lightHistoryIcon,
            ])}
          />
          <CaretDown
            {...stylex.props([
              styles.historyCaret,
              isDark ? styles.darkHistoryCaret : styles.lightHistoryIcon,
              isDropdownOpen && styles.historyCaretOpen,
            ])}
          />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        variant="app"
        align="start"
        side={layout === "floating" ? "right" : "bottom"}
        sideOffset={4}
        avoidCollisions
        collisionPadding={8}
        sx={styles.historyMenu}
      >
        <AppFloatingPanel sx={styles.historyPanel}>
          <div {...stylex.props(styles.historyHeadingContainer)}>
            <h4 {...stylex.props(styles.historyHeading)}>Recent Chats</h4>
          </div>
          {recentChatGroups.length > 0 ? (
            <div {...stylex.props(styles.historyList)}>
              {recentChatGroups.map((chatGroup) => (
                <ChatGroupItem
                  key={chatGroup.id}
                  chatGroup={chatGroup}
                  isActive={chatGroup.id === currentChatGroupId}
                  onSelect={(id) => {
                    onSelectChat(id);
                    setIsDropdownOpen(false);
                  }}
                />
              ))}
            </div>
          ) : (
            <div {...stylex.props(styles.emptyHistory)}>
              <ChatCircle {...stylex.props(styles.emptyHistoryIcon)} />
              <p {...stylex.props(styles.emptyHistoryText)}>
                <Trans>No recent chats</Trans>
              </p>
            </div>
          )}
        </AppFloatingPanel>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function ChatGroupItem({
  chatGroup,
  isActive,
  onSelect,
}: {
  chatGroup: ChatGroupRecord;
  isActive: boolean;
  onSelect: (groupId: string) => void;
}) {
  const formattedTime = chatGroup.createdAt
    ? formatDistanceToNow(new Date(chatGroup.createdAt), {
        addSuffix: true,
      })
    : "";

  return (
    <Button
      variant="ghost"
      onClick={() => onSelect(chatGroup.id)}
      sx={[
        styles.chatGroupItem,
        isActive ? styles.activeChatGroupItem : styles.inactiveChatGroupItem,
      ]}
    >
      <div {...stylex.props(styles.chatGroupContent)}>
        <div {...stylex.props(styles.chatGroupIconSlot)}>
          <ChatCircle {...stylex.props(styles.chatGroupIcon)} />
        </div>
        <div {...stylex.props(styles.chatGroupText)}>
          <div
            {...stylex.props([
              styles.chatGroupTitle,
              isActive
                ? styles.activeChatGroupTitle
                : styles.inactiveChatGroupTitle,
            ])}
          >
            {chatGroup.title}
          </div>
          <div {...stylex.props(styles.chatGroupTime)}>{formattedTime}</div>
        </div>
      </div>
    </Button>
  );
}

const styles = stylex.create({
  toolbar: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    height: "100%",
    minWidth: 0,
    width: "100%",
  },
  floatingToolbar: {
    paddingInline: "0.75rem",
  },
  rightPanelToolbar: {
    paddingLeft: "0.75rem",
    paddingRight: "0.25rem",
  },
  historySlot: {
    alignItems: "center",
    display: "flex",
    flex: "1",
    gap: "0.25rem",
    minWidth: 0,
  },
  actions: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    gap: 0,
  },
  actionButton: {
    backgroundColor: "transparent",
    borderRadius: radii.full,
    color: colors.mutedForeground,
    height: "2rem",
    width: "2rem",
  },
  compactActionButton: {
    height: "1.75rem",
    width: "1.75rem",
  },
  darkToolbarButton: {
    backgroundColor: {
      default: "transparent",
      ":active": `color-mix(in oklab, ${colors.primaryForeground} 18%, transparent)`,
      ":focus-visible": `color-mix(in oklab, ${colors.primaryForeground} 14%, transparent)`,
      ":hover": `color-mix(in oklab, ${colors.primaryForeground} 14%, transparent)`,
    },
    color: {
      default: `color-mix(in oklab, ${colors.primaryForeground} 60%, transparent)`,
      ":focus-visible": colors.primaryForeground,
      ":hover": colors.primaryForeground,
    },
  },
  lightToolbarButton: {
    backgroundColor: {
      default: "transparent",
      ":active": colors.muted,
      ":focus-visible": `color-mix(in oklab, ${colors.muted} 80%, transparent)`,
      ":hover": `color-mix(in oklab, ${colors.muted} 80%, transparent)`,
    },
    color: {
      default: colors.mutedForeground,
      ":focus-visible": colors.foreground,
      ":hover": colors.foreground,
    },
  },
  historyTrigger: {
    borderRadius: radii.full,
    flexShrink: 0,
    gap: "0.375rem",
    height: "2rem",
    marginLeft: "-0.5rem",
    paddingBlock: 0,
    paddingInline: "0.625rem",
    transitionDuration: "150ms",
    transitionProperty: "color, background-color, border-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "auto",
  },
  compactHistoryTrigger: {
    height: "1.75rem",
  },
  darkHistoryTrigger: {
    backgroundColor: {
      default: "transparent",
      ":hover": `color-mix(in oklab, ${colors.primaryForeground} 14%, transparent)`,
      ':is([data-state="open"])': `color-mix(in oklab, ${colors.primaryForeground} 14%, transparent)`,
    },
    color: {
      default: `color-mix(in oklab, ${colors.primaryForeground} 70%, transparent)`,
      ":hover": colors.primaryForeground,
      ':is([data-state="open"])': colors.primaryForeground,
    },
  },
  lightHistoryTrigger: {
    backgroundColor: {
      default: "transparent",
      ":hover": `color-mix(in oklab, ${colors.muted} 80%, transparent)`,
      ':is([data-state="open"])': `color-mix(in oklab, ${colors.muted} 80%, transparent)`,
    },
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
      ':is([data-state="open"])': colors.foreground,
    },
  },
  historyIcon: {
    height: "1rem",
    width: "1rem",
  },
  darkHistoryIcon: {
    color: `color-mix(in oklab, ${colors.primaryForeground} 70%, transparent)`,
  },
  lightHistoryIcon: {
    color: colors.mutedForeground,
  },
  historyCaret: {
    flexShrink: 0,
    height: "0.875rem",
    transitionDuration: "200ms",
    transitionProperty: "transform",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "0.875rem",
  },
  darkHistoryCaret: {
    color: `color-mix(in oklab, ${colors.primaryForeground} 50%, transparent)`,
  },
  historyCaretOpen: {
    transform: "rotate(180deg)",
  },
  historyMenu: {
    maxHeight:
      "min(20rem, var(--radix-dropdown-menu-content-available-height))",
    maxWidth: "var(--radix-dropdown-menu-content-available-width)",
    overflowY: "auto",
    width: "18rem",
  },
  historyPanel: {
    padding: "0.375rem",
  },
  historyHeadingContainer: {
    paddingBlock: "0.375rem",
    paddingInline: "0.5rem",
  },
  historyHeading: {
    color: colors.mutedForeground,
    fontSize: "0.625rem",
    fontWeight: 600,
    letterSpacing: "0.05em",
    textTransform: "uppercase",
  },
  historyList: {
    display: "flex",
    flexDirection: "column",
    gap: "0.125rem",
  },
  emptyHistory: {
    paddingBlock: "1.5rem",
    paddingInline: "0.75rem",
    textAlign: "center",
  },
  emptyHistoryIcon: {
    color: `color-mix(in oklab, ${colors.mutedForeground} 70%, transparent)`,
    height: "1.5rem",
    marginBottom: "0.375rem",
    marginInline: "auto",
    width: "1.5rem",
  },
  emptyHistoryText: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  chatGroupItem: {
    height: "auto",
    justifyContent: "flex-start",
    paddingBlock: "0.375rem",
    paddingInline: "0.625rem",
    width: "100%",
  },
  activeChatGroupItem: {
    backgroundColor: {
      default: colors.muted,
      ":hover": colors.accent,
    },
    boxShadow: shadows.sm,
  },
  inactiveChatGroupItem: {
    backgroundColor: {
      default: "transparent",
      ":active": colors.muted,
      ":hover": colors.accent,
    },
  },
  chatGroupContent: {
    alignItems: "center",
    display: "flex",
    gap: "0.625rem",
    width: "100%",
  },
  chatGroupIconSlot: {
    flexShrink: 0,
  },
  chatGroupIcon: {
    color: colors.mutedForeground,
    height: "0.875rem",
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "0.875rem",
  },
  chatGroupText: {
    flex: "1",
    minWidth: 0,
    textAlign: "left",
  },
  chatGroupTitle: {
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  activeChatGroupTitle: {
    color: colors.foreground,
  },
  inactiveChatGroupTitle: {
    color: colors.mutedForeground,
  },
  chatGroupTime: {
    color: colors.mutedForeground,
    fontSize: "0.6875rem",
    marginTop: "0.125rem",
  },
});

const toolbarButtonStyles = {
  dark: styles.darkToolbarButton,
  light: styles.lightToolbarButton,
};
