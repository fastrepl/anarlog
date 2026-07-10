import { useLingui } from "@lingui/react/macro";
import { CornerDownLeft } from "lucide-react";
import React, { useState } from "react";

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
    <div className="px-2 py-2">
      <form onSubmit={handleSubmit}>
        <div className="border-border bg-accent/50 focus-within:bg-accent flex h-8 w-full items-center gap-2 rounded-lg border px-3 transition-colors">
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t`Add person`}
            className="placeholder:text-muted-foreground w-full bg-transparent text-sm focus:outline-hidden"
            autoFocus
          />
          {name.trim() && (
            <button
              type="submit"
              className="text-muted-foreground hover:text-muted-foreground shrink-0 transition-colors"
              aria-label={t`Add person`}
            >
              <CornerDownLeft className="size-4" />
            </button>
          )}
        </div>
      </form>
    </div>
  );
}
