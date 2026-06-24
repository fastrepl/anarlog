import { Trans, useLingui } from "@lingui/react/macro";
import {
  ChevronDown,
  History,
  MessageCircle,
  PanelRight,
  PictureInPicture2,
  Plus,
  X,
} from "lucide-react";
import { useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";
import { cn, formatDistanceToNow } from "@hypr/utils";

import * as main from "~/store/tinybase/store/main";

export function ChatToolbarControls({
  currentChatGroupId,
  layout = "floating",
  onClose,
  onNewChat,
  onOpenFloating,
  onOpenRightPanel,
  onSelectChat,
  surface = "light",
}: {
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

  return (
    <div
      className={cn(["flex h-full w-full min-w-0 items-center gap-2", "px-3"])}
    >
      <div className="flex min-w-0 flex-1 items-center gap-1">
        <ChatGroups
          currentChatGroupId={currentChatGroupId}
          onSelectChat={onSelectChat}
          surface={surface}
        />
      </div>
      <div
        data-chat-toolbar-actions
        className="flex shrink-0 items-center gap-0"
      >
        <ChatActionButton
          icon={<Plus size={16} />}
          label={t`New chat`}
          onClick={onNewChat}
          className={
            isDark ? darkToolbarButtonClassName : lightToolbarButtonClassName
          }
        />
        {isRightPanel ? (
          <>
            <ChatActionButton
              icon={<PictureInPicture2 size={16} />}
              label={t`Float chat`}
              onClick={onOpenFloating ?? (() => {})}
              className={
                isDark
                  ? darkToolbarButtonClassName
                  : lightToolbarButtonClassName
              }
            />
            <ChatActionButton
              icon={<X size={16} />}
              label={t`Close chat`}
              onClick={onClose ?? (() => {})}
              className={
                isDark
                  ? darkToolbarButtonClassName
                  : lightToolbarButtonClassName
              }
            />
          </>
        ) : (
          <>
            <ChatActionButton
              icon={<PanelRight size={16} />}
              label={t`Open in right panel`}
              onClick={onOpenRightPanel ?? (() => {})}
              className={
                isDark
                  ? darkToolbarButtonClassName
                  : lightToolbarButtonClassName
              }
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
  currentChatGroupId,
  onSelectChat,
  surface = "light",
}: {
  currentChatGroupId: string | undefined;
  onSelectChat: (chatGroupId: string) => void;
  surface?: "light" | "dark";
}) {
  const { t } = useLingui();
  const [isDropdownOpen, setIsDropdownOpen] = useState(false);
  const isDark = surface === "dark";

  const recentChatGroupIds = main.UI.useSortedRowIds(
    "chat_groups",
    "created_at",
    true,
    0,
    5,
    main.STORE_ID,
  );

  return (
    <DropdownMenu open={isDropdownOpen} onOpenChange={setIsDropdownOpen}>
      <DropdownMenuTrigger asChild>
        <Button
          aria-label={t`Chat history`}
          variant="ghost"
          size="sm"
          className={cn([
            "group -ml-2 h-8 w-auto shrink-0 gap-1.5 rounded-full px-2.5 py-0 transition-colors",
            isDark
              ? "text-primary-foreground/70 hover:bg-primary-foreground/14 hover:text-primary-foreground data-[state=open]:bg-primary-foreground/14 data-[state=open]:text-primary-foreground"
              : "text-muted-foreground hover:bg-muted/80 hover:text-foreground data-[state=open]:bg-muted/80 data-[state=open]:text-foreground",
          ])}
        >
          <History
            className={cn([
              "h-4 w-4",
              isDark ? "text-primary-foreground/70" : "text-muted-foreground",
            ])}
          />
          <ChevronDown
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
        side="bottom"
        sideOffset={4}
        avoidCollisions={false}
        className="w-72 max-w-[calc(100vw-2rem)]"
      >
        <AppFloatingPanel className="max-h-80 overflow-y-auto p-1.5">
          <div className="px-2 py-1.5">
            <h4 className="text-muted-foreground text-[10px] font-semibold tracking-wider uppercase">
              Recent Chats
            </h4>
          </div>
          {recentChatGroupIds.length > 0 ? (
            <div className="flex flex-col gap-0.5">
              {recentChatGroupIds.map((groupId) => (
                <ChatGroupItem
                  key={groupId}
                  groupId={groupId}
                  isActive={groupId === currentChatGroupId}
                  onSelect={(id) => {
                    onSelectChat(id);
                    setIsDropdownOpen(false);
                  }}
                />
              ))}
            </div>
          ) : (
            <div className="px-3 py-6 text-center">
              <MessageCircle className="text-muted-foreground/70 mx-auto mb-1.5 h-6 w-6" />
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
  groupId,
  isActive,
  onSelect,
}: {
  groupId: string;
  isActive: boolean;
  onSelect: (groupId: string) => void;
}) {
  const chatGroup = main.UI.useRow("chat_groups", groupId, main.STORE_ID);

  if (!chatGroup) {
    return null;
  }

  const formattedTime = chatGroup.created_at
    ? formatDistanceToNow(new Date(chatGroup.created_at), {
        addSuffix: true,
      })
    : "";

  return (
    <Button
      variant="ghost"
      onClick={() => onSelect(groupId)}
      className={cn([
        "group h-auto w-full justify-start px-2.5 py-1.5",
        isActive
          ? "bg-muted hover:bg-accent shadow-xs"
          : "hover:bg-accent active:bg-muted",
      ])}
    >
      <div className="flex w-full items-center gap-2.5">
        <div className="shrink-0">
          <MessageCircle
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
