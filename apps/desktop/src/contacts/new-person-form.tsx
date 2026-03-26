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
    <div className="p-2">
      <form onSubmit={handleSubmit}>
        <div className="flex w-full items-center gap-2 rounded-xs border border-neutral-200 bg-neutral-50 px-2 py-1.5">
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Add person"
            className="w-full bg-transparent text-sm placeholder:text-neutral-400 focus:outline-hidden"
            autoFocus
          />
          {name.trim() && (
            <button
              type="submit"
              className="shrink-0 text-neutral-500 transition-colors hover:text-neutral-700"
              aria-label="Add person"
            >
              <CornerDownLeft className="size-4" />
            </button>
          )}
        </div>
      </form>
    </div>
  );
}
