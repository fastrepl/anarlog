import type { ApiKeyInfo } from "@anlg/plugin-local-api";
import { Button } from "@anlg/ui/components/ui/button";

export function ApiKeyRow({
  apiKey,
  onRevoke,
}: {
  apiKey: Pick<
    ApiKeyInfo,
    "id" | "name" | "key_prefix" | "created_at" | "last_used_at"
  >;
  onRevoke: () => void;
}) {
  return (
    <li className="flex items-center justify-between gap-3 text-sm">
      <div className="flex min-w-0 items-center gap-2">
        <code className="bg-muted shrink-0 rounded-md px-1.5 py-0.5 text-xs">
          {apiKey.key_prefix}…
        </code>
        <span className="truncate">{apiKey.name}</span>
      </div>
      <div className="flex shrink-0 items-center gap-2">
        <span className="text-muted-foreground text-xs">
          {apiKey.last_used_at
            ? `Last used ${apiKey.last_used_at.slice(0, 10)}`
            : `Created ${apiKey.created_at.slice(0, 10)}`}
        </span>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="text-destructive h-7"
          onClick={onRevoke}
        >
          Revoke
        </Button>
      </div>
    </li>
  );
}
