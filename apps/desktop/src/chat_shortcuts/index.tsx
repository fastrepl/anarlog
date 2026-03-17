import { Globe, MessageSquare, Plus, Search, X } from "lucide-react";
import { useCallback, useMemo, useState } from "react";

import type { ChatShortcut } from "@hypr/store";
import { Button } from "@hypr/ui/components/ui/button";
import { Switch } from "@hypr/ui/components/ui/switch";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";
import { cn } from "@hypr/utils";

import { ChatShortcutDetailsColumn } from "./details";

import { StandardTabWrapper } from "~/shared/main";
import { type TabItem, TabItemBase } from "~/shared/tabs";
import { ResourceListLayout, useWebResources } from "~/shared/ui/resource-list";
import * as main from "~/store/tinybase/store/main";
import { type Tab, useTabs } from "~/store/zustand/tabs";

export const TabItemChatShortcut: TabItem<
  Extract<Tab, { type: "chat_shortcuts" }>
> = ({
  tab,
  tabIndex,
  handleCloseThis,
  handleSelectThis,
  handleCloseOthers,
  handleCloseAll,
  handlePinThis,
  handleUnpinThis,
}) => {
  return (
    <TabItemBase
      icon={<MessageSquare className="h-4 w-4" />}
      title="Shortcuts"
      selected={tab.active}
      pinned={tab.pinned}
      tabIndex={tabIndex}
      handleCloseThis={() => handleCloseThis(tab)}
      handleSelectThis={() => handleSelectThis(tab)}
      handleCloseOthers={handleCloseOthers}
      handleCloseAll={handleCloseAll}
      handlePinThis={() => handlePinThis(tab)}
      handleUnpinThis={() => handleUnpinThis(tab)}
    />
  );
};

export function TabContentChatShortcut({
  tab,
}: {
  tab: Extract<Tab, { type: "chat_shortcuts" }>;
}) {
  return (
    <StandardTabWrapper>
      <ChatShortcutView tab={tab} />
    </StandardTabWrapper>
  );
}

export type WebShortcut = {
  slug: string;
  title: string;
  description: string;
  category: string;
  targets?: string[];
  prompt: string;
};

type UserShortcut = ChatShortcut & { id: string };

function useChatShortcuts(): UserShortcut[] {
  const shortcuts = main.UI.useResultTable(
    main.QUERIES.visibleChatShortcuts,
    main.STORE_ID,
  );

  return useMemo(() => {
    return Object.entries(shortcuts as Record<string, ChatShortcut>).map(
      ([id, shortcut]) => ({
        id,
        ...shortcut,
      }),
    );
  }, [shortcuts]);
}

function ChatShortcutView({
  tab,
}: {
  tab: Extract<Tab, { type: "chat_shortcuts" }>;
}) {
  const updateTabState = useTabs((state) => state.updateChatShortcutsTabState);
  const { user_id } = main.UI.useValues(main.STORE_ID);

  const userShortcuts = useChatShortcuts();
  const { data: webShortcuts = [], isLoading: isWebLoading } =
    useWebResources<WebShortcut>("shortcuts");

  const { selectedMineId, selectedWebIndex } = tab.state;
  const isWebMode = tab.state.isWebMode ?? userShortcuts.length === 0;

  const setIsWebMode = useCallback(
    (value: boolean) => {
      updateTabState(tab, {
        isWebMode: value,
        selectedMineId: null,
        selectedWebIndex: null,
      });
    },
    [updateTabState, tab],
  );

  const setSelectedMineId = useCallback(
    (id: string | null) => {
      updateTabState(tab, {
        ...tab.state,
        selectedMineId: id,
        selectedWebIndex: null,
      });
    },
    [updateTabState, tab],
  );

  const setSelectedWebIndex = useCallback(
    (index: number | null) => {
      updateTabState(tab, {
        ...tab.state,
        selectedMineId: null,
        selectedWebIndex: index,
      });
    },
    [updateTabState, tab],
  );

  const selectedWebShortcut =
    selectedWebIndex !== null ? (webShortcuts[selectedWebIndex] ?? null) : null;

  const setRow = main.UI.useSetRowCallback(
    "chat_shortcuts",
    (p: {
      id: string;
      user_id: string;
      created_at: string;
      title: string;
      content: string;
    }) => p.id,
    (p: {
      id: string;
      user_id: string;
      created_at: string;
      title: string;
      content: string;
    }) => ({
      user_id: p.user_id,
      created_at: p.created_at,
      title: p.title,
      content: p.content,
    }),
    [],
    main.STORE_ID,
  );

  const handleCloneShortcut = useCallback(
    (shortcut: WebShortcut) => {
      if (!user_id) return;

      const newId = crypto.randomUUID();
      const now = new Date().toISOString();

      setRow({
        id: newId,
        user_id,
        created_at: now,
        title: shortcut.title,
        content: shortcut.prompt,
      });

      updateTabState(tab, {
        isWebMode: false,
        selectedMineId: newId,
        selectedWebIndex: null,
      });
    },
    [user_id, setRow, updateTabState, tab],
  );

  const handleAddNew = useCallback(() => {
    if (!user_id) return;

    const newId = crypto.randomUUID();
    const now = new Date().toISOString();

    setRow({ id: newId, user_id, created_at: now, title: "", content: "" });

    updateTabState(tab, {
      isWebMode: false,
      selectedMineId: newId,
      selectedWebIndex: null,
    });
  }, [user_id, setRow, updateTabState, tab]);

  return (
    <ResourceListLayout
      listColumn={
        <ShortcutListColumn
          isWebMode={isWebMode}
          setIsWebMode={setIsWebMode}
          userShortcuts={userShortcuts}
          webShortcuts={webShortcuts}
          isWebLoading={isWebLoading}
          selectedMineId={selectedMineId}
          selectedWebIndex={selectedWebIndex}
          setSelectedMineId={setSelectedMineId}
          setSelectedWebIndex={setSelectedWebIndex}
          onAddNew={handleAddNew}
        />
      }
      detailsColumn={
        <ChatShortcutDetailsColumn
          isWebMode={isWebMode}
          selectedMineId={selectedMineId}
          selectedWebShortcut={selectedWebShortcut}
          setSelectedMineId={setSelectedMineId}
          handleCloneShortcut={handleCloneShortcut}
        />
      }
    />
  );
}

function ShortcutListColumn({
  isWebMode,
  setIsWebMode,
  userShortcuts,
  webShortcuts,
  isWebLoading,
  selectedMineId,
  selectedWebIndex,
  setSelectedMineId,
  setSelectedWebIndex,
  onAddNew,
}: {
  isWebMode: boolean;
  setIsWebMode: (value: boolean) => void;
  userShortcuts: UserShortcut[];
  webShortcuts: WebShortcut[];
  isWebLoading: boolean;
  selectedMineId: string | null;
  selectedWebIndex: number | null;
  setSelectedMineId: (id: string | null) => void;
  setSelectedWebIndex: (index: number | null) => void;
  onAddNew: () => void;
}) {
  const [search, setSearch] = useState("");
  const [showSearch, setShowSearch] = useState(false);

  const getMineTitle = (item: UserShortcut) => {
    if (item.title?.trim()) return item.title;
    const content = item.content?.trim();
    if (!content) return "Untitled shortcut";
    return content.length > 50 ? content.slice(0, 50) + "..." : content;
  };

  const filteredMine = useMemo(() => {
    if (!search.trim()) return userShortcuts;
    const q = search.toLowerCase();
    return userShortcuts.filter((s) => s.content?.toLowerCase().includes(q));
  }, [userShortcuts, search]);

  const filteredWeb = useMemo(() => {
    if (!search.trim()) return webShortcuts;
    const q = search.toLowerCase();
    return webShortcuts.filter(
      (s) =>
        s.title?.toLowerCase().includes(q) ||
        s.description?.toLowerCase().includes(q) ||
        s.category?.toLowerCase().includes(q),
    );
  }, [webShortcuts, search]);

  const items = isWebMode ? filteredWeb : filteredMine;

  return (
    <div className="flex h-full w-full flex-col">
      <div className="border-b border-neutral-200">
        <div className="flex h-12 items-center justify-between py-2 pr-1 pl-3">
          <h3 className="text-sm font-medium">Shortcuts</h3>
          <div className="flex items-center gap-1">
            <Tooltip>
              <TooltipTrigger asChild>
                <div className="flex items-center gap-1.5 px-2">
                  <Globe size={14} className="text-neutral-400" />
                  <Switch
                    size="sm"
                    checked={isWebMode}
                    onCheckedChange={setIsWebMode}
                  />
                </div>
              </TooltipTrigger>
              <TooltipContent side="bottom">
                {isWebMode
                  ? "Showing community shortcuts"
                  : "Showing your shortcuts"}
              </TooltipContent>
            </Tooltip>
            <Button
              onClick={() => {
                if (showSearch) setSearch("");
                setShowSearch(!showSearch);
              }}
              size="icon"
              variant="ghost"
              className="text-neutral-600 hover:text-black"
            >
              <Search size={16} />
            </Button>
            <Button
              onClick={onAddNew}
              size="icon"
              variant="ghost"
              className="text-neutral-600 hover:text-black"
            >
              <Plus size={16} />
            </Button>
          </div>
        </div>
        {showSearch && (
          <div className="flex h-12 items-center gap-2 border-t border-neutral-200 bg-white px-3">
            <Search className="h-4 w-4 text-neutral-400" />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  setSearch("");
                  setShowSearch(false);
                }
              }}
              placeholder="Search shortcuts..."
              className="w-full bg-transparent text-sm placeholder:text-neutral-400 focus:outline-hidden"
              autoFocus
            />
            {search && (
              <button
                onClick={() => setSearch("")}
                className="rounded-xs p-1 hover:bg-neutral-100"
              >
                <X className="h-4 w-4 text-neutral-400" />
              </button>
            )}
          </div>
        )}
      </div>

      <div className="flex-1 overflow-y-auto p-2">
        {isWebMode && isWebLoading ? (
          <div className="flex flex-col gap-2">
            {[0, 1, 2, 3].map((i) => (
              <div key={i} className="animate-pulse rounded-md px-3 py-2">
                <div className="h-4 w-3/4 rounded-xs bg-neutral-200" />
                <div className="mt-1.5 h-3 w-1/2 rounded-xs bg-neutral-100" />
              </div>
            ))}
          </div>
        ) : items.length === 0 ? (
          <div className="py-8 text-center text-neutral-500">
            <MessageSquare
              size={32}
              className="mx-auto mb-2 text-neutral-300"
            />
            <p className="text-sm">
              {search
                ? "No shortcuts found"
                : isWebMode
                  ? "No community shortcuts"
                  : "No shortcuts yet"}
            </p>
          </div>
        ) : isWebMode ? (
          filteredWeb.map((item, index) => (
            <button
              key={`web-${index}`}
              onClick={() => setSelectedWebIndex(index)}
              className={cn([
                "w-full rounded-md border px-3 py-2 text-left text-sm hover:bg-neutral-100",
                selectedWebIndex === index
                  ? "border-neutral-500 bg-neutral-100"
                  : "border-transparent",
              ])}
            >
              <div className="flex items-center gap-2">
                <MessageSquare className="h-4 w-4 shrink-0 text-neutral-500" />
                <div className="min-w-0 flex-1">
                  <div className="truncate font-medium">
                    {item.title || "Untitled"}
                    {item.category && (
                      <span className="ml-1 font-mono text-xs text-stone-400">
                        ({item.category})
                      </span>
                    )}
                  </div>
                  {item.description && (
                    <div className="truncate text-xs text-neutral-500">
                      {item.description}
                    </div>
                  )}
                </div>
              </div>
            </button>
          ))
        ) : (
          filteredMine.map((item) => (
            <button
              key={item.id}
              onClick={() => setSelectedMineId(item.id)}
              className={cn([
                "w-full rounded-md border px-3 py-2 text-left text-sm hover:bg-neutral-100",
                selectedMineId === item.id
                  ? "border-neutral-500 bg-neutral-100"
                  : "border-transparent",
              ])}
            >
              <div className="flex items-center gap-2">
                <MessageSquare className="h-4 w-4 shrink-0 text-neutral-500" />
                <div className="min-w-0 flex-1">
                  <div className="truncate font-medium">
                    {getMineTitle(item)}
                  </div>
                </div>
              </div>
            </button>
          ))
        )}
      </div>
    </div>
  );
}
