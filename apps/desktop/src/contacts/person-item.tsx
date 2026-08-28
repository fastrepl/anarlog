import { PushPin } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import React, { useCallback } from "react";

import { colors } from "@anlg/design-system/tokens.stylex";

import { contactItemStyles } from "./contact-item-styles";

import { ContactImage } from "~/contacts/contact-avatar";
import { type HumanRecord, toggleContactPin } from "~/contacts/queries";
import { ContactFacehash } from "~/contacts/shared";
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
      {person.avatarDataUrl ? (
        <ContactImage src={person.avatarDataUrl} size={32} />
      ) : (
        <ContactFacehash name={facehashName} size={32} />
      )}
      <div {...stylex.props(contactItemStyles.body)}>
        <div {...stylex.props(styles.name)}>
          {personName || personEmail || "Unnamed"}
        </div>
        {personEmail && personName && (
          <div {...stylex.props(styles.email)}>{personEmail}</div>
        )}
      </div>
      <button
        onClick={handleTogglePin}
        {...stylex.props([
          contactItemStyles.pin,
          isPinned ? contactItemStyles.pinned : contactItemStyles.unpinned,
        ])}
        aria-label={isPinned ? "Unpin contact" : "Pin contact"}
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
  email: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  name: {
    alignItems: "center",
    display: "flex",
    fontWeight: 500,
    gap: "0.25rem",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
});
