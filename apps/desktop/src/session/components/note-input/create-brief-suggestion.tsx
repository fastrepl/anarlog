import { useLingui } from "@lingui/react/macro";
import { Sparkle } from "@phosphor-icons/react";

import { cn } from "@anlg/utils";

export function CreateBriefSuggestion({ onCreate }: { onCreate: () => void }) {
  const { t } = useLingui();
  const label = t`Want me to create a brief to help you prepare?`;

  return (
    <div className="mb-6">
      <p className="text-muted-foreground flex h-8 items-center text-xs">
        {t`Prepare for this meeting`}
      </p>
      <button
        type="button"
        aria-label={label}
        onClick={onCreate}
        className={cn([
          "hover:bg-accent focus-visible:bg-accent pointer-events-auto -ml-2 flex h-8 w-fit max-w-full items-center gap-2 rounded-md px-2 text-left",
          "text-muted-foreground hover:text-foreground focus-visible:text-foreground transition-colors focus-visible:outline-hidden",
        ])}
      >
        <Sparkle aria-hidden className="size-4 shrink-0" />
        <span className="min-w-0 truncate text-sm font-medium">{label}</span>
      </button>
    </div>
  );
}
