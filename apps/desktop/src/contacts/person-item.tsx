import { Pin } from "lucide-react";
import React, { useCallback } from "react";

import { cn } from "@hypr/utils";

import { type HumanRecord, toggleContactPin } from "~/contacts/queries";
import { ContactFacehash, getContactBgClass } from "~/contacts/shared";
import { useNativeContextMenu } from "~/shared/hooks/useNativeContextMenu";

export function PersonItem({
  person,
  active,
  onClick,
  onDelete,
}: {
  person: HumanRecord;
  active: boolean;
  onClick: () => void;
  onDelete?: (id: string) => void;
}) {
  const isPinned = Boolean(person.pinned);
  const personName = person.name;
  const personEmail = person.email;
  const facehashName = personName || personEmail || person.id;
  const bgClass = getContactBgClass(facehashName);

  const togglePin = useCallback(() => {
    void toggleContactPin("human", person.id).catch((error) => {
      console.error("[contacts] failed to toggle contact pin", error);
    });
  }, [person.id]);

  const showContextMenu = useNativeContextMenu([
    {
      id: "toggle-pin-person",
      text: isPinned ? "Unpin Contact" : "Pin Contact",
      action: togglePin,
    },
    {
      id: "delete-person",
      text: "Delete Contact",
      action: () => onDelete?.(person.id),
    },
  ]);

  const handleTogglePin = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      togglePin();
    },
    [togglePin],
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
        "group flex w-full items-center gap-2 overflow-hidden rounded-lg px-3 py-2 text-left text-sm transition-colors select-none",
        active ? "bg-accent" : "hover:bg-accent/50",
      ])}
    >
      <div className={cn(["shrink-0 rounded-full", bgClass])}>
        <ContactFacehash
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
          <div className="text-muted-foreground truncate text-xs">
            {personEmail}
          </div>
        )}
      </div>
      <button
        onClick={handleTogglePin}
        className={cn([
          "shrink-0 rounded-xs p-1 transition-colors",
          isPinned
            ? "text-blue-600 hover:text-blue-700"
            : "text-muted-foreground/70 hover:text-muted-foreground opacity-0 group-hover:opacity-100",
        ])}
        aria-label={isPinned ? "Unpin contact" : "Pin contact"}
      >
        <Pin className="size-3.5" fill={isPinned ? "currentColor" : "none"} />
      </button>
    </div>
  );
}
