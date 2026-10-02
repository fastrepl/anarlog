import { Trans, useLingui } from "@lingui/react/macro";
import { useQuery } from "@tanstack/react-query";
import { useState } from "react";

import { commands as tracingCommands } from "@anlg/plugin-tracing";
import { CaretDown, CircleNotch } from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import { cn } from "@anlg/utils";

async function readCloudsyncLog() {
  const result = await tracingCommands.logContent(true);
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data?.split(/\n(?=\d{4}-\d{2}-\d{2}T)/).reverse() ?? [];
}

export function SyncLog() {
  const { t } = useLingui();
  const [open, setOpen] = useState(false);
  const logQuery = useQuery({
    queryKey: ["app-log", "cloudsync"],
    queryFn: readCloudsyncLog,
    enabled: open,
    refetchInterval: open ? 5_000 : false,
  });

  return (
    <div className="border-border/60 overflow-hidden rounded-xl border">
      <button
        type="button"
        aria-label={open ? t`Hide sync log` : t`View sync log`}
        aria-expanded={open}
        className="hover:bg-muted/40 flex w-full items-center justify-between gap-4 px-4 py-3 text-left transition-colors"
        onClick={() => setOpen((value) => !value)}
      >
        <div>
          <h3 className="text-xs font-medium">
            <Trans>Sync log</Trans>
          </h3>
          <p className="text-muted-foreground mt-0.5 text-[11px]">
            <Trans>Recent CloudSync entries from the app log.</Trans>
          </p>
        </div>
        <CaretDown
          className={cn([
            "text-muted-foreground size-3.5 transition-transform",
            open && "rotate-180",
          ])}
        />
      </button>

      {open && (
        <div className="border-border/60 border-t px-4 py-3">
          {logQuery.isError ? (
            <div className="flex items-center justify-between gap-3">
              <p className="text-xs text-red-500">
                <Trans>Could not load sync logs.</Trans>
              </p>
              <Button
                variant="outline"
                size="sm"
                disabled={logQuery.isFetching}
                onClick={() => void logQuery.refetch()}
              >
                <Trans>Retry</Trans>
              </Button>
            </div>
          ) : logQuery.isPending ? (
            <div className="flex justify-center py-2">
              <CircleNotch
                aria-label={t`Loading sync logs`}
                className="text-muted-foreground size-4 animate-spin"
              />
            </div>
          ) : logQuery.data.length ? (
            <ol className="divide-border/60 max-h-64 divide-y overflow-y-auto font-mono text-[11px] leading-5">
              {logQuery.data.map((line, index) => (
                <li
                  key={`${index}-${line}`}
                  className="py-2 break-words whitespace-pre-wrap first:pt-0 last:pb-0"
                >
                  {line}
                </li>
              ))}
            </ol>
          ) : (
            <p className="text-muted-foreground py-2 text-center text-xs">
              <Trans>No sync activity yet.</Trans>
            </p>
          )}
        </div>
      )}
    </div>
  );
}
