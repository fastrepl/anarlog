import { Buildings, PushPin } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import React, { useCallback } from "react";

import { contactItemStyles } from "./contact-item-styles";

import { ContactImage } from "~/contacts/contact-avatar";
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
      data-contact-item
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
      {...stylex.props([
        contactItemStyles.item,
        active ? contactItemStyles.active : contactItemStyles.inactive,
      ])}
    >
      {organization.avatarDataUrl ? (
        <ContactImage src={organization.avatarDataUrl} size={32} />
      ) : (
        <div {...stylex.props(contactItemStyles.avatarFallback)}>
          <Buildings {...stylex.props(contactItemStyles.avatarIcon)} />
        </div>
      )}
      <div {...stylex.props(contactItemStyles.body)}>
        <div {...stylex.props(styles.name)}>{organization.name}</div>
      </div>
      <button
        onClick={handleTogglePin}
        {...stylex.props([
          contactItemStyles.pin,
          isPinned ? contactItemStyles.pinned : contactItemStyles.unpinned,
        ])}
        aria-label={isPinned ? "Unpin organization" : "Pin organization"}
      >
        <PushPin
          {...stylex.props(contactItemStyles.icon)}
          weight={isPinned ? "fill" : "regular"}
        />
      </button>
    </div>
  );
}

const styles = stylex.create({
  name: {
    fontWeight: 500,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
});
