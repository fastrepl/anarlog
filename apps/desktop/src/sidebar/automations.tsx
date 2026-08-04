import { Lightning, MagnifyingGlass, Plus, X } from "@phosphor-icons/react";
import { useMemo, useState } from "react";

import { Button } from "@anlg/ui/components/ui/button";
import { cn, formatDistanceToNow } from "@anlg/utils";

import { CustomSidebarHeader } from "./custom-sidebar-header";

import { useChatContext } from "~/chat/state/chat-context";
import { useChatGroups } from "~/chat/store/queries";

export function AutomationsNav() {
  const [search, setSearch] = useState("");
  const automations = useChatGroups("automations");
  const selectedAutomationId = useChatContext(
    (state) => state.chatByScope.automations.groupId,
  );
  const selectChat = useChatContext((state) => state.selectChat);
  const startNewChat = useChatContext((state) => state.startNewChat);

  const filteredAutomations = useMemo(() => {
    const query = search.trim().toLowerCase();
    if (!query) {
      return automations;
    }

    return automations.filter((automation) =>
      automation.title.toLowerCase().includes(query),
    );
  }, [automations, search]);

  return (
    <div className="flex h-full flex-col overflow-hidden pb-2">
      <CustomSidebarHeader title="Automations">
        <Button
          type="button"
          size="icon"
          variant="ghost"
          className="text-muted-foreground relative z-[60] hover:text-black"
          aria-label="New automation"
          onClick={() => startNewChat("automations")}
        >
          <Plus size={16} />
        </Button>
      </CustomSidebarHeader>

      <div className="pb-2">
        <div
          className={cn([
            "border-border bg-accent/50 flex h-8 w-full shrink-0 items-center gap-2 rounded-lg border px-3",
            "focus-within:bg-accent transition-colors",
          ])}
        >
          <MagnifyingGlass className="text-muted-foreground h-4 w-4 shrink-0" />
          <input
            type="text"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                setSearch("");
              }
            }}
            placeholder="Search automations..."
            className="placeholder:text-muted-foreground min-w-0 flex-1 bg-transparent text-sm placeholder:text-sm focus:outline-hidden"
          />
          {search ? (
            <button
              type="button"
              onClick={() => setSearch("")}
              className={cn([
                "size-4 shrink-0",
                "text-muted-foreground hover:text-foreground",
                "transition-colors",
              ])}
              aria-label="Clear search"
            >
              <X className="size-4" />
            </button>
          ) : null}
        </div>
      </div>

      <div className="scrollbar-hide min-h-0 flex-1 overflow-y-auto pt-1">
        {filteredAutomations.length > 0 ? (
          filteredAutomations.map((automation) => {
            const selected = automation.id === selectedAutomationId;
            const createdAt = automation.createdAt
              ? formatDistanceToNow(new Date(automation.createdAt), {
                  addSuffix: true,
                })
              : "";

            return (
              <button
                key={automation.id}
                type="button"
                aria-current={selected ? "page" : undefined}
                onClick={() => selectChat("automations", automation.id)}
                className={cn([
                  "w-full rounded-lg px-3 py-2 text-left text-sm transition-colors select-none",
                  selected ? "bg-accent" : "hover:bg-accent/50",
                ])}
              >
                <span className="flex items-center gap-2">
                  <Lightning
                    className="size-4 shrink-0 text-violet-500"
                    weight="fill"
                  />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate font-medium">
                      {automation.title}
                    </span>
                    {createdAt ? (
                      <span className="text-muted-foreground mt-0.5 block truncate text-xs">
                        {createdAt}
                      </span>
                    ) : null}
                  </span>
                </span>
              </button>
            );
          })
        ) : (
          <div className="text-muted-foreground px-3 py-8 text-center">
            <Lightning
              size={32}
              className="text-muted-foreground/70 mx-auto mb-2"
            />
            <p className="text-sm">
              {search ? "No automations found" : "No automations yet"}
            </p>
          </div>
        )}
      </div>
    </div>
  );
}
