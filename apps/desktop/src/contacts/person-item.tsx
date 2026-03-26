import { Facehash } from "facehash";
import { Pin } from "lucide-react";
import React, { useCallback } from "react";

import { cn } from "@hypr/utils";

import { getContactBgClass } from "~/contacts/shared";
import { useNativeContextMenu } from "~/shared/hooks/useNativeContextMenu";
import * as main from "~/store/tinybase/store/main";

export function PersonItem({
  humanId,
  active,
  onClick,
  onDelete,
}: {
  humanId: string;
  active: boolean;
  onClick: () => void;
  onDelete?: (id: string) => void;
}) {
  const person = main.UI.useRow("humans", humanId, main.STORE_ID);
  const isPinned = Boolean(person.pinned);
  const personName = String(person.name ?? "");
  const personEmail = String(person.email ?? "");
  const facehashName = personName || personEmail || humanId;
  const bgClass = getContactBgClass(facehashName);

  const store = main.UI.useStore(main.STORE_ID);

  const showContextMenu = useNativeContextMenu([
    {
      id: "delete-person",
      text: "Delete Contact",
      action: () => onDelete?.(humanId),
    },
  ]);

  const handleTogglePin = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (!store) return;

      const currentPinned = store.getCell("humans", humanId, "pinned");
      if (currentPinned) {
        store.setPartialRow("humans", humanId, {
          pinned: false,
          pin_order: 0,
        });
      } else {
        const allHumans = store.getTable("humans");
        const allOrgs = store.getTable("organizations");
        const maxHumanOrder = Object.values(allHumans).reduce((max, h) => {
          const order = (h.pin_order as number | undefined) ?? 0;
          return Math.max(max, order);
        }, 0);
        const maxOrgOrder = Object.values(allOrgs).reduce((max, o) => {
          const order = (o.pin_order as number | undefined) ?? 0;
          return Math.max(max, order);
        }, 0);
        store.setPartialRow("humans", humanId, {
          pinned: true,
          pin_order: Math.max(maxHumanOrder, maxOrgOrder) + 1,
        });
      }
    },
    [store, humanId],
  );

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onClick}
      onContextMenu={showContextMenu}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClick();
        }
      }}
      className={cn([
        "group flex w-full items-center gap-2 overflow-hidden rounded-md border bg-white px-3 py-2 text-left text-sm transition-colors hover:bg-neutral-100",
        active ? "border-neutral-500 bg-neutral-100" : "border-transparent",
      ])}
    >
      <div className={cn(["shrink-0 rounded-full", bgClass])}>
        <Facehash
          name={facehashName}
          size={32}
          interactive={true}
          showInitial={true}
          colorClasses={[bgClass]}
        />
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1 truncate font-medium">
          {personName || personEmail || "Unnamed"}
        </div>
        {personEmail && personName && (
          <div className="truncate text-xs text-neutral-500">{personEmail}</div>
        )}
      </div>
      <button
        onClick={handleTogglePin}
        className={cn([
          "shrink-0 rounded-xs p-1 transition-colors",
          isPinned
            ? "text-blue-600 hover:text-blue-700"
            : "text-neutral-300 opacity-0 group-hover:opacity-100 hover:text-neutral-500",
        ])}
        aria-label={isPinned ? "Unpin contact" : "Pin contact"}
      >
        <Pin className="size-3.5" fill={isPinned ? "currentColor" : "none"} />
      </button>
    </div>
  );
}
