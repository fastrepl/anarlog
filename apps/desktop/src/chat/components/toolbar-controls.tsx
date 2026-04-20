import { ChevronDown, MessageCircle, Plus } from "lucide-react";
import { useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@hypr/ui/components/ui/dropdown-menu";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn, formatDistanceToNow } from "@hypr/utils";

import * as main from "~/store/tinybase/store/main";

export function ChatToolbarControls({
  currentChatGroupId,
  onNewChat,
  onSelectChat,
}: {
  currentChatGroupId: string | undefined;
  onNewChat: () => void;
  onSelectChat: (chatGroupId: string) => void;
}) {
  return (
    <div
      data-tauri-drag-region
      className="flex h-full min-w-0 items-center gap-1"
    >
      <ChatGroups
        currentChatGroupId={currentChatGroupId}
        onSelectChat={onSelectChat}
      />
      <ChatActionButton
        icon={<Plus size={16} />}
        onClick={onNewChat}
        title="New chat"
      />
    </div>
  );
}

function ChatActionButton({
  icon,
  title,
  onClick,
}: {
  icon: React.ReactNode;
  title: string;
  onClick: () => void;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          onClick={onClick}
          title={title}
          size="icon"
          variant="ghost"
          className="text-neutral-600"
        >
          {icon}
        </Button>
      </TooltipTrigger>
      <TooltipContent side="bottom">{title}</TooltipContent>
    </Tooltip>
  );
}

function ChatGroups({
  currentChatGroupId,
  onSelectChat,
}: {
  currentChatGroupId: string | undefined;
  onSelectChat: (chatGroupId: string) => void;
}) {
  const [isDropdownOpen, setIsDropdownOpen] = useState(false);

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
            "group flex h-8 max-w-64 min-w-0 justify-start gap-2 px-2",
            "text-neutral-700",
          ])}
        >
          <h3 className="min-w-0 flex-1 truncate text-xs font-medium text-neutral-700">
            {currentChatTitle || "Ask Charlie anything"}
          </h3>
          <ChevronDown
            className={cn([
              "h-3.5 w-3.5 shrink-0 text-neutral-400 transition-transform duration-200",
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
            <h4 className="text-[10px] font-semibold tracking-wider text-neutral-500 uppercase">
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
              <MessageCircle className="mx-auto mb-1.5 h-6 w-6 text-neutral-300" />
              <p className="text-xs text-neutral-400">No recent chats</p>
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
          ? "bg-neutral-100 shadow-xs hover:bg-neutral-100"
          : "hover:bg-neutral-50 active:bg-neutral-100",
      ])}
    >
      <div className="flex w-full items-center gap-2.5">
        <div className="shrink-0">
          <MessageCircle
            className={cn([
              "h-3.5 w-3.5 transition-colors",
              isActive
                ? "text-neutral-700"
                : "text-neutral-400 group-hover:text-neutral-600",
            ])}
          />
        </div>
        <div className="min-w-0 flex-1 text-left">
          <div
            className={cn([
              "truncate text-sm font-medium",
              isActive ? "text-neutral-900" : "text-neutral-700",
            ])}
          >
            {chatGroup.title}
          </div>
          <div className="mt-0.5 text-[11px] text-neutral-500">
            {formattedTime}
          </div>
        </div>
      </div>
    </Button>
  );
}
