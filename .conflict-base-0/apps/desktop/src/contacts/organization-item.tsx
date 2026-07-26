import { Building2, Pin } from "lucide-react";
import React, { useCallback } from "react";

import { cn } from "@hypr/utils";

import { type OrganizationRecord, toggleContactPin } from "~/contacts/queries";
import { useNativeContextMenu } from "~/shared/hooks/useNativeContextMenu";

export function OrganizationItem({
  organization,
  active,
  onClick,
  onDelete,
}: {
  organization: OrganizationRecord;
  active: boolean;
  onClick: () => void;
  onDelete?: (id: string) => void;
}) {
  const isPinned = Boolean(organization.pinned);

  const togglePin = useCallback(() => {
    void toggleContactPin("organization", organization.id).catch((error) => {
      console.error("[contacts] failed to toggle organization pin", error);
    });
  }, [organization.id]);

  const showContextMenu = useNativeContextMenu([
    {
      id: "toggle-pin-org",
      text: isPinned ? "Unpin Organization" : "Pin Organization",
      action: togglePin,
    },
    {
      id: "delete-org",
      text: "Delete Organization",
      action: () => onDelete?.(organization.id),
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
      <div className="bg-muted flex h-8 w-8 shrink-0 items-center justify-center rounded-full">
        <Building2 className="text-muted-foreground h-4 w-4" />
      </div>
      <div className="min-w-0 flex-1">
        <div className="truncate font-medium">{organization.name}</div>
      </div>
      <button
        onClick={handleTogglePin}
        className={cn([
          "shrink-0 rounded-xs p-1 transition-colors",
          isPinned
            ? "text-blue-600 hover:text-blue-700"
            : "text-muted-foreground/70 hover:text-muted-foreground opacity-0 group-hover:opacity-100",
        ])}
        aria-label={isPinned ? "Unpin organization" : "Pin organization"}
      >
        <Pin className="size-3.5" fill={isPinned ? "currentColor" : "none"} />
      </button>
    </div>
  );
}
