import { useLingui } from "@lingui/react/macro";
import { CornerDownLeft } from "lucide-react";
import React, { useState } from "react";

import * as main from "~/store/tinybase/store/main";

export function NewPersonForm({
  onSave,
  onCancel,
}: {
  onSave: (humanId: string) => void;
  onCancel: () => void;
}) {
  const { t } = useLingui();
  const [name, setName] = useState("");
  const userId = main.UI.useValue("user_id", main.STORE_ID);

  const createHuman = main.UI.useSetRowCallback(
    "humans",
    (p: { name: string; humanId: string }) => p.humanId,
    (p: { name: string; humanId: string }) => ({
      user_id: userId || "",
      created_at: new Date().toISOString(),
      name: p.name,
      email: "",
      phone: "",
      org_id: "",
      job_title: "",
      linkedin_username: "",
      memo: "",
      pinned: false,
    }),
    [userId],
    main.STORE_ID,
  );

  const handleAdd = () => {
    const humanId = crypto.randomUUID();
    createHuman({ humanId, name: name.trim() });
    setName("");
    onSave(humanId);
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (name.trim()) {
      handleAdd();
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      if (name.trim()) {
        handleAdd();
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
