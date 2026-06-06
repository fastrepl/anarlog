import { cn } from "@hypr/utils";

import { displayPath } from "./path-utils";

export function ObsidianVaultList({
  vaults,
  home,
  disabled,
  onSelect,
  actionLabel,
}: {
  vaults: Array<{ path: string }>;
  home: string | undefined;
  disabled?: boolean;
  onSelect: (path: string) => void;
  actionLabel?: string;
}) {
  if (vaults.length === 0) return null;

  return (
    <div className="flex flex-col gap-1.5">
      <p className="text-muted-foreground text-xs font-medium">
        Detected Obsidian vaults
      </p>
      {vaults.map((vault) => (
        <button
          key={vault.path}
          disabled={disabled}
          onClick={() => onSelect(vault.path)}
          className={cn([
            "border-border bg-muted text-muted-foreground hover:border-border hover:bg-accent flex items-center gap-2 rounded-full border px-3 py-2 text-left text-sm transition-colors disabled:opacity-50",
          ])}
        >
          <img
            src="/assets/obsidian-icon.svg"
            className="size-4 shrink-0"
            aria-hidden="true"
          />
          <span className="min-w-0 flex-1 truncate">
            {displayPath(vault.path, home)}
          </span>
          {actionLabel && (
            <span className="text-muted-foreground shrink-0 text-xs">
              {actionLabel}
            </span>
          )}
        </button>
      ))}
    </div>
  );
}
