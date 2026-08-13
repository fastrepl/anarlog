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
import { useState } from "react";

import { Button } from "@anlg/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { cn, formatDistanceToNow } from "@anlg/utils";

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
  const isDark = surface === "dark";
  const isRightPanel = layout === "right-panel";
  const actionButtonClassName = cn([
    isDark ? darkToolbarButtonClassName : lightToolbarButtonClassName,
    isRightPanel && "size-7",
  ]);

  return (
    <div
      data-tauri-drag-region={isRightPanel || undefined}
      className={cn([
        "flex h-full w-full min-w-0 items-center gap-2",
        isRightPanel ? "pr-1 pl-3" : "px-3",
      ])}
    >
      <div
        data-tauri-drag-region={isRightPanel || undefined}
        className="flex min-w-0 flex-1 items-center gap-1"
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
        className="flex shrink-0 items-center gap-0"
      >
        <ChatActionButton
          icon={<Plus size={16} />}
          label={t`New chat`}
          onClick={onNewChat}
          className={actionButtonClassName}
        />
        {isRightPanel ? (
          <>
            <ChatActionButton
              icon={<PictureInPicture size={16} />}
              label={t`Float chat`}
              onClick={onOpenFloating ?? (() => {})}
              className={actionButtonClassName}
            />
            <ChatActionButton
              icon={<X size={16} />}
              label={t`Close chat`}
              onClick={onClose ?? (() => {})}
              className={actionButtonClassName}
            />
          </>
        ) : (
          <>
            <ChatActionButton
              icon={<SidebarSimple size={16} />}
              label={t`Open in right panel`}
              onClick={onOpenRightPanel ?? (() => {})}
              className={actionButtonClassName}
            />
          </>
        )}
      </div>
    </div>
  );
}

const darkToolbarButtonClassName =
  "size-8 bg-transparent text-primary-foreground/60 hover:!bg-primary-foreground/14 hover:!text-primary-foreground focus-visible:!bg-primary-foreground/14 focus-visible:!text-primary-foreground active:!bg-primary-foreground/18";

const lightToolbarButtonClassName =
  "size-8 bg-transparent text-muted-foreground hover:!bg-muted/80 hover:!text-foreground focus-visible:!bg-muted/80 focus-visible:!text-foreground active:!bg-muted";

function ChatActionButton({
  className,
  icon,
  label,
  onClick,
}: {
  className?: string;
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <Button
      aria-label={label}
      data-tauri-drag-region="false"
      onClick={onClick}
      size="icon"
      variant="ghost"
      className={cn(["text-muted-foreground rounded-full", className])}
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
          className={cn([
            "group -ml-2 h-8 w-auto shrink-0 gap-1.5 rounded-full px-2.5 py-0 transition-colors",
            layout === "right-panel" && "h-7",
            isDark
              ? "text-primary-foreground/70 hover:bg-primary-foreground/14 hover:text-primary-foreground data-[state=open]:bg-primary-foreground/14 data-[state=open]:text-primary-foreground"
              : "text-muted-foreground hover:bg-muted/80 hover:text-foreground data-[state=open]:bg-muted/80 data-[state=open]:text-foreground",
          ])}
        >
          <ClockCounterClockwise
            className={cn([
              "h-4 w-4",
              isDark ? "text-primary-foreground/70" : "text-muted-foreground",
            ])}
          />
          <CaretDown
            className={cn([
              "h-3.5 w-3.5 shrink-0 transition-transform duration-200",
              isDark ? "text-primary-foreground/50" : "text-muted-foreground",
              isDropdownOpen && "rotate-180",
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
        className="max-h-[min(20rem,var(--radix-dropdown-menu-content-available-height))] w-72 max-w-[var(--radix-dropdown-menu-content-available-width)] overflow-y-auto"
      >
        <AppFloatingPanel className="p-1.5">
          <div className="px-2 py-1.5">
            <h4 className="text-muted-foreground text-[10px] font-semibold tracking-wider uppercase">
              Recent Chats
            </h4>
          </div>
          {recentChatGroups.length > 0 ? (
            <div className="flex flex-col gap-0.5">
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
            <div className="px-3 py-6 text-center">
              <ChatCircle className="text-muted-foreground/70 mx-auto mb-1.5 h-6 w-6" />
              <p className="text-muted-foreground text-xs">
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
      className={cn([
        "group h-auto w-full justify-start px-2.5 py-1.5",
        isActive
          ? "bg-muted hover:bg-accent shadow-xs"
          : "hover:bg-accent active:bg-muted",
      ])}
    >
      <div className="flex w-full items-center gap-2.5">
        <div className="shrink-0">
          <ChatCircle
            className={cn([
              "h-3.5 w-3.5 transition-colors",
              isActive
                ? "text-muted-foreground"
                : "text-muted-foreground group-hover:text-muted-foreground",
            ])}
          />
        </div>
        <div className="min-w-0 flex-1 text-left">
          <div
            className={cn([
              "truncate text-sm font-medium",
              isActive ? "text-foreground" : "text-muted-foreground",
            ])}
          >
            {chatGroup.title}
          </div>
          <div className="text-muted-foreground mt-0.5 text-[11px]">
            {formattedTime}
          </div>
        </div>
      </div>
    </Button>
  );
}
