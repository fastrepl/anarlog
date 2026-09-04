import { useLingui } from "@lingui/react/macro";

import { Sparkle } from "@anlg/ui/components/icons";
import { cn } from "@anlg/utils";

export function CreateBriefSuggestion({ onCreate }: { onCreate: () => void }) {
  const { t } = useLingui();
  const label = t`Create a brief to prepare this meeting`;

  return (
    <button
      type="button"
      aria-label={label}
      onClick={onCreate}
      className={cn([
        "hover:bg-accent focus-visible:bg-accent pointer-events-auto mb-6 -ml-2 flex h-8 w-fit max-w-full items-center gap-2 rounded-md px-2 text-left",
        "text-muted-foreground hover:text-foreground focus-visible:text-foreground transition-colors focus-visible:outline-hidden",
      ])}
    >
      <Sparkle aria-hidden className="size-4 shrink-0" />
      <span className="min-w-0 truncate text-sm font-medium">{label}</span>
    </button>
  );
}
