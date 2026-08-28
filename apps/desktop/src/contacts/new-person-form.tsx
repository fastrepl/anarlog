import { useLingui } from "@lingui/react/macro";
import { ArrowElbowDownLeft } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import React, { useState } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";

import { createHuman } from "~/contacts/queries";

export function NewPersonForm({
  onSave,
  onCancel,
}: {
  onSave: (humanId: string) => void;
  onCancel: () => void;
}) {
  const { t } = useLingui();
  const [name, setName] = useState("");

  const handleAdd = async () => {
    try {
      const humanId = await createHuman({ name: name.trim() });
      setName("");
      onSave(humanId);
    } catch (error) {
      console.error("[contacts] failed to create contact", error);
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (name.trim()) {
      void handleAdd();
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      if (name.trim()) {
        void handleAdd();
      }
    }
    if (e.key === "Escape") {
      onCancel();
    }
  };

  return (
    <div {...stylex.props(styles.root)}>
      <form onSubmit={handleSubmit}>
        <div {...stylex.props(styles.field)}>
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t`Add person`}
            {...stylex.props(styles.input)}
            autoFocus
          />
          {name.trim() && (
            <button
              type="submit"
              {...stylex.props(styles.submit)}
              aria-label={t`Add person`}
            >
              <ArrowElbowDownLeft {...stylex.props(styles.icon)} />
            </button>
          )}
        </div>
      </form>
    </div>
  );
}

const styles = stylex.create({
  field: {
    alignItems: "center",
    backgroundColor: {
      default: `color-mix(in srgb, ${colors.accent} 50%, transparent)`,
      ":focus-within": colors.accent,
    },
    borderColor: colors.border,
    borderRadius: radii.lg,
    borderStyle: "solid",
    borderWidth: "1px",
    display: "flex",
    gap: "0.5rem",
    height: "2rem",
    paddingInline: "0.75rem",
    transitionDuration: "150ms",
    transitionProperty: "background-color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
    width: "100%",
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  input: {
    "::placeholder": {
      color: colors.mutedForeground,
    },
    backgroundColor: "transparent",
    fontSize: "0.875rem",
    lineHeight: "1.25rem",
    outline: {
      default: null,
      ":focus": "none",
    },
    width: "100%",
  },
  root: {
    padding: "0.5rem",
  },
  submit: {
    color: colors.mutedForeground,
    flexShrink: 0,
    transitionDuration: "150ms",
    transitionProperty: "color",
    transitionTimingFunction: "cubic-bezier(0.4, 0, 0.2, 1)",
  },
});
