import { useLingui } from "@lingui/react/macro";

import { MagnifyingGlass, X } from "@anlg/ui/components/icons";
import { useSquircleRef } from "@anlg/ui/hooks/use-squircle";

export function filterProviders<
  T extends { id: string; displayName: string; description?: string },
>(providers: readonly T[], query: string): T[] {
  const normalizedQuery = query.trim().toLocaleLowerCase();
  if (!normalizedQuery) {
    return [...providers];
  }

  return providers.filter((provider) =>
    `${provider.displayName} ${provider.id} ${provider.description ?? ""}`
      .toLocaleLowerCase()
      .includes(normalizedQuery),
  );
}

export function ProviderSearch({
  value,
  onChange,
}: {
  value: string;
  onChange: (value: string) => void;
}) {
  const { t } = useLingui();
  const ref = useSquircleRef<HTMLDivElement>();

  return (
    <div
      ref={ref}
      className="border-border bg-muted/50 focus-within:bg-accent ml-auto flex h-8 w-56 max-w-[55%] items-center gap-2 rounded-lg border px-2.5 transition-colors"
    >
      <MagnifyingGlass className="text-muted-foreground size-3.5 shrink-0" />
      <input
        type="search"
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Escape") onChange("");
        }}
        placeholder={t`Search providers...`}
        aria-label={t`Search providers`}
        className="placeholder:text-muted-foreground min-w-0 flex-1 bg-transparent text-sm focus:outline-hidden [&::-webkit-search-cancel-button]:hidden"
      />
      {value ? (
        <button
          type="button"
          onClick={() => onChange("")}
          className="text-muted-foreground hover:text-foreground shrink-0 transition-colors"
          aria-label={t`Clear search`}
        >
          <X className="size-3.5" />
        </button>
      ) : null}
    </div>
  );
}
