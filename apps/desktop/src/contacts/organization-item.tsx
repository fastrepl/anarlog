import { Building2, Pin } from "lucide-react";
import React, { useCallback } from "react";

import { cn } from "@hypr/utils";

import { useOrganization, useToggleOrganizationPin } from "~/contacts/hooks";
import { useNativeContextMenu } from "~/shared/hooks/useNativeContextMenu";

export function OrganizationItem({
  organizationId,
  active,
  onClick,
  onDelete,
}: {
  organizationId: string;
  active: boolean;
  onClick: () => void;
  onDelete?: (id: string) => void;
}) {
  const organization = useOrganization(organizationId);
  const isPinned = organization?.pinned ?? false;
  const togglePin = useToggleOrganizationPin(organizationId);

  const showContextMenu = useNativeContextMenu([
    {
      id: "toggle-pin-org",
      text: isPinned ? "Unpin Organization" : "Pin Organization",
      action: togglePin,
    },
    {
      id: "delete-org",
      text: "Delete Organization",
      action: () => onDelete?.(organizationId),
    },
  ]);

  const handleTogglePin = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      togglePin();
    },
    [togglePin],
  );

  if (!organization) {
    return null;
  }

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
        active ? "bg-neutral-200" : "hover:bg-neutral-200/50",
      ])}
    >
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-neutral-100">
        <Building2 className="h-4 w-4 text-neutral-500" />
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
            : "text-neutral-300 opacity-0 group-hover:opacity-100 hover:text-neutral-500",
        ])}
        aria-label={isPinned ? "Unpin organization" : "Pin organization"}
      >
        <Pin className="size-3.5" fill={isPinned ? "currentColor" : "none"} />
      </button>
    </div>
  );
}
