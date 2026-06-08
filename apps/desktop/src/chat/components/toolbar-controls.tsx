import {
  ChevronDown,
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
  const isDark = surface === "dark";
  const isRightPanel = layout === "right-panel";

  return (
    <div
      className={cn([
        "flex h-full w-full min-w-0 items-center gap-2",
        isRightPanel ? "pr-1 pl-3" : isDark ? "px-2" : "pr-2 pl-2",
      ])}
    >
      <div className="flex min-w-0 flex-1 items-center gap-1">
        <ChatGroups
          currentChatGroupId={currentChatGroupId}
          onSelectChat={onSelectChat}
          surface={surface}
        />
      </div>
      <div className="flex shrink-0 items-center gap-1">
        <ChatActionButton
          icon={<Plus size={16} />}
          label="New chat"
          onClick={onNewChat}
          className={isDark ? darkToolbarButtonClassName : undefined}
        />
        {isRightPanel ? (
          <>
            <ChatActionButton
              icon={<PictureInPicture2 size={16} />}
              label="Float chat"
              onClick={onOpenFloating ?? (() => {})}
              className={isDark ? darkToolbarButtonClassName : undefined}
            />
            <ChatActionButton
              icon={<X size={16} />}
              label="Close chat"
              onClick={onClose ?? (() => {})}
              className={isDark ? darkToolbarButtonClassName : undefined}
            />
          </>
        ) : (
          <ChatActionButton
            icon={<PanelRight size={16} />}
            label="Open in right panel"
            onClick={onOpenRightPanel ?? (() => {})}
            className={isDark ? darkToolbarButtonClassName : undefined}
          />
        )}
      </div>
    </div>
  );
}

const darkToolbarButtonClassName =
  "size-8 bg-transparent text-primary-foreground/60 hover:!bg-primary-foreground/7 hover:!text-primary-foreground focus-visible:!bg-primary-foreground/7 focus-visible:!text-primary-foreground active:!bg-primary-foreground/10";

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
  const [isDropdownOpen, setIsDropdownOpen] = useState(false);
  const isDark = surface === "dark";

  const currentChatTitle = main.UI.useCell(
    "chat_groups",
    currentChatGroupId || "",
    "title",
    main.STORE_ID,
  );
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
          variant="ghost"
          className={cn([
            "group flex h-8 max-w-full min-w-0 justify-start gap-1.5 py-0 text-left",
            "-ml-2 px-2",
            isDark
              ? "text-primary-foreground hover:bg-primary-foreground/7 hover:text-primary-foreground data-[state=open]:bg-primary-foreground/7 w-fit rounded-full"
              : "text-foreground hover:bg-accent hover:text-foreground data-[state=open]:bg-accent w-fit rounded-full",
          ])}
        >
          <h3
            className={cn([
              "max-w-64 min-w-0 truncate text-left font-medium",
              isDark
                ? "text-primary-foreground text-[15px]"
                : "text-foreground text-[15px]",
            ])}
          >
            {currentChatTitle || "Ask Anarlog AI anything"}
          </h3>
          <ChevronDown
            className={cn([
              "h-3.5 w-3.5 shrink-0 transition-transform duration-200",
              isDark ? "text-primary-foreground/60" : "text-muted-foreground",
              isDropdownOpen && "rotate-180",
            ])}
          />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        variant="app"
        align="start"
        sideOffset={0}
        className="w-72"
      >
        <AppFloatingPanel className="flex flex-col gap-0.5 p-1.5">
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
              <p className="text-muted-foreground text-xs">No recent chats</p>
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
