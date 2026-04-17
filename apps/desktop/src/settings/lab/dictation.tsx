import {
  ChevronDownIcon,
  ChevronUpIcon,
  MoreVerticalIcon,
  PencilIcon,
  ToggleLeftIcon,
} from "lucide-react";
import { useEffect, useState } from "react";

import { Button } from "@hypr/ui/components/ui/button";
import { Kbd } from "@hypr/ui/components/ui/kbd";
import { Textarea } from "@hypr/ui/components/ui/textarea";
import { cn } from "@hypr/utils";

import { SettingsPageTitle } from "~/settings/page-title";

const MODIFIER_KEYS = new Set(["Meta", "Control", "Alt", "Shift"]);

function formatCombo(e: KeyboardEvent): string | null {
  const parts: string[] = [];
  if (e.metaKey) {
    parts.push("⌘");
  }
  if (e.ctrlKey) {
    parts.push("Ctrl");
  }
  if (e.altKey) {
    parts.push("Alt");
  }
  if (e.shiftKey) {
    parts.push("Shift");
  }
  if (!MODIFIER_KEYS.has(e.key)) {
    const k = e.key === " " ? "Space" : e.key;
    parts.push(k.length === 1 ? k.toUpperCase() : k);
  }
  return parts.length === 0 ? null : parts.join("+");
}

function KeyCaptureInline({
  onSave,
  onCancel,
}: {
  onSave: (value: string) => void;
  onCancel: () => void;
}) {
  const [value, setValue] = useState<string | null>(null);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onCancel();
        return;
      }
      e.preventDefault();
      const combo = formatCombo(e);
      if (combo) {
        setValue(combo);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [onCancel]);

  return (
    <div className="flex items-center gap-2">
      <div
        className={cn([
          "flex min-h-8 min-w-28 items-center justify-center rounded-md border px-3 py-1",
          "border-blue-400 bg-blue-50",
        ])}
      >
        {value ? (
          <Kbd>{value}</Kbd>
        ) : (
          <span className="text-xs text-neutral-500">Press keys…</span>
        )}
      </div>
      <Button size="sm" variant="outline" onClick={onCancel}>
        Cancel
      </Button>
      <Button
        size="sm"
        disabled={!value}
        onClick={() => value && onSave(value)}
      >
        Save
      </Button>
    </div>
  );
}

function DictationKeybindings({
  keybinding,
  onChange,
}: {
  keybinding: string;
  onChange: (value: string) => void;
}) {
  const [editing, setEditing] = useState(false);

  return (
    <div>
      <div className="mb-2 flex items-center gap-3">
        <span className="text-xs font-semibold tracking-wider text-neutral-500">
          KEYBINDINGS
        </span>
        <div className="h-px flex-1 bg-neutral-200" />
      </div>
      <div className="rounded-lg border border-neutral-200 bg-white">
        <div className="flex items-center justify-between gap-3 px-4 py-3">
          <div className="flex items-center gap-3">
            <ToggleLeftIcon className="size-5 text-neutral-500" />
            <span className="text-sm font-medium">Activate</span>
          </div>
          {editing ? (
            <KeyCaptureInline
              onSave={(v) => {
                onChange(v);
                setEditing(false);
              }}
              onCancel={() => setEditing(false)}
            />
          ) : (
            <div className="flex items-center gap-2">
              <Kbd>{keybinding}</Kbd>
              <button
                type="button"
                onClick={() => setEditing(true)}
                className="rounded p-1 text-neutral-500 hover:bg-neutral-100 hover:text-neutral-900"
                aria-label="Edit keybinding"
              >
                <PencilIcon className="size-4" />
              </button>
              <button
                type="button"
                className="rounded p-1 text-neutral-500 hover:bg-neutral-100 hover:text-neutral-900"
                aria-label="More options"
              >
                <MoreVerticalIcon className="size-4" />
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function DictationPlayground({ keybinding }: { keybinding: string }) {
  const [open, setOpen] = useState(true);

  return (
    <div className="rounded-lg border border-neutral-200 bg-white">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="flex w-full items-center justify-between gap-3 px-4 py-3"
      >
        <div className="flex items-center gap-2 text-sm text-neutral-700">
          <span>Hold</span>
          <Kbd>{keybinding}</Kbd>
          <span>to talk into any textbox.</span>
        </div>
        <div className="flex items-center gap-1 text-xs text-neutral-500">
          <span>{open ? "Hide" : "Show"}</span>
          {open ? (
            <ChevronUpIcon className="size-4" />
          ) : (
            <ChevronDownIcon className="size-4" />
          )}
        </div>
      </button>
      {open && (
        <div className="border-t border-neutral-200 px-4 py-3">
          <Textarea
            placeholder="Test it out here. How is your day going?"
            className="min-h-24"
          />
        </div>
      )}
    </div>
  );
}

export function DictationSettings() {
  const [keybinding, setKeybinding] = useState("Fn");

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title="Dictation" />
      <DictationKeybindings keybinding={keybinding} onChange={setKeybinding} />
      <DictationPlayground keybinding={keybinding} />
    </div>
  );
}
