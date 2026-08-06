import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowSquareOut,
  CircleNotch,
  DownloadSimple,
  MagnifyingGlass,
} from "@phosphor-icons/react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { open as selectFiles } from "@tauri-apps/plugin-dialog";
import { useState } from "react";

import { commands as importerCommands } from "@anlg/plugin-importer";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { Button } from "@anlg/ui/components/ui/button";
import { Input } from "@anlg/ui/components/ui/input";
import { cn } from "@anlg/utils";

import { detectImportSources } from "./detection";
import {
  MEETING_IMPORT_PROVIDERS,
  type MeetingImportProvider,
} from "./providers";
import {
  EMPTY_MEETING_IMPORT_HISTORY,
  importMeetingFiles,
  useMeetingImportHistory,
} from "./queries";
import { pauseCompetingApplicationTermination } from "./termination-pause";

import { useMountEffect } from "~/shared/hooks/useMountEffect";

const IMPORT_EXTENSIONS = [
  "csv",
  "json",
  "md",
  "markdown",
  "srt",
  "txt",
  "vtt",
];

export function MeetingImportScreen({
  compact = false,
  onContinue,
}: {
  compact?: boolean;
  onContinue?: () => void;
}) {
  const { t } = useLingui();
  const [search, setSearch] = useState("");
  const detectionQuery = useQuery({
    queryKey: ["meeting-import-sources"],
    queryFn: detectImportSources,
    refetchOnMount: "always",
  });
  useMountEffect(pauseCompetingApplicationTermination);
  const historyQuery = useMeetingImportHistory();
  const history = historyQuery.data ?? EMPTY_MEETING_IMPORT_HISTORY;
  const detectedIds = new Set(
    detectionQuery.data?.map((provider) => provider.id) ?? [],
  );
  const providers = [...MEETING_IMPORT_PROVIDERS].sort((left, right) => {
    const detectedOrder =
      Number(detectedIds.has(right.id)) - Number(detectedIds.has(left.id));
    return detectedOrder || left.name.localeCompare(right.name);
  });
  const query = search.trim().toLowerCase();
  const visibleProviders = query
    ? providers.filter((provider) =>
        `${provider.name} ${provider.access}`.toLowerCase().includes(query),
      )
    : providers;

  const importMutation = useMutation({
    mutationFn: async (provider: MeetingImportProvider) => {
      const selection = await selectFiles({
        title: t`Choose ${provider.name} export files`,
        multiple: true,
        directory: false,
        filters: [
          {
            name: t`Meeting exports`,
            extensions: IMPORT_EXTENSIONS,
          },
        ],
      });
      const paths = Array.isArray(selection)
        ? selection
        : selection
          ? [selection]
          : [];
      if (paths.length === 0) return null;

      const filesResult = await importerCommands.readTextFiles(paths);
      if (filesResult.status === "error") throw new Error(filesResult.error);
      return importMeetingFiles(provider.id, filesResult.data);
    },
  });

  return (
    <div className={cn(["flex flex-col gap-4", compact && "max-w-3xl"])}>
      <div className="flex flex-col gap-1">
        <p className="text-muted-foreground text-sm">
          <Trans>
            Anarlog detects installed meeting assistants and imports their
            official JSON, CSV, Markdown, text, VTT, or SRT exports.
          </Trans>
        </p>
        {detectionQuery.isLoading ? (
          <p className="text-muted-foreground flex items-center gap-2 text-xs">
            <CircleNotch className="size-3.5 animate-spin" />
            <Trans>Checking installed meeting assistants…</Trans>
          </p>
        ) : detectionQuery.error ? (
          <p className="text-destructive text-xs">
            {detectionQuery.error.message}
          </p>
        ) : null}
      </div>

      <div className="relative">
        <MagnifyingGlass className="text-muted-foreground pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2" />
        <Input
          value={search}
          onChange={(event) => setSearch(event.target.value)}
          placeholder={t`Search 30 supported apps`}
          className="rounded-full pl-9"
        />
      </div>

      {importMutation.error ? (
        <p className="text-destructive text-sm">
          {importMutation.error.message}
        </p>
      ) : null}
      {importMutation.data ? (
        <div className="border-border bg-card rounded-xl border px-4 py-3 text-sm">
          <Trans>
            Imported {importMutation.data.imported} meetings. Matched{" "}
            {importMutation.data.matched} unchanged meetings;{" "}
            {importMutation.data.conflicts} conflicts;{" "}
            {importMutation.data.errors} errors.
          </Trans>
        </div>
      ) : null}

      <div
        className={cn([
          "border-border bg-card divide-border divide-y overflow-hidden rounded-2xl border",
          compact && "max-h-80 overflow-y-auto",
        ])}
      >
        {visibleProviders.map((provider) => {
          const detected = detectedIds.has(provider.id);
          const importing =
            importMutation.isPending &&
            importMutation.variables.id === provider.id;
          const lastRun = history.find((run) => run.providerId === provider.id);

          return (
            <div
              key={provider.id}
              className="flex min-h-16 items-center gap-3 px-4 py-3"
            >
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="truncate text-sm font-medium">
                    {provider.name}
                  </span>
                  {detected ? (
                    <span className="bg-primary/10 text-primary rounded-full px-2 py-0.5 text-[10px] font-medium">
                      <Trans>Detected</Trans>
                    </span>
                  ) : null}
                  <span className="text-muted-foreground rounded-full border px-2 py-0.5 text-[10px]">
                    {provider.access}
                  </span>
                </div>
                {lastRun ? (
                  <p className="text-muted-foreground mt-1 text-xs">
                    <Trans>
                      Last import: {lastRun.imported} added, {lastRun.matched}{" "}
                      unchanged
                    </Trans>
                  </p>
                ) : null}
              </div>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={() =>
                  void openerCommands.openUrl(provider.helpUrl, null)
                }
              >
                <ArrowSquareOut className="size-3.5" />
                <Trans>Export help</Trans>
              </Button>
              <Button
                type="button"
                size="sm"
                disabled={importMutation.isPending}
                onClick={() => importMutation.mutate(provider)}
              >
                {importing ? (
                  <CircleNotch className="size-3.5 animate-spin" />
                ) : (
                  <DownloadSimple className="size-3.5" />
                )}
                <Trans>Import</Trans>
              </Button>
            </div>
          );
        })}
      </div>

      {onContinue && importMutation.data ? (
        <Button
          type="button"
          className="w-fit rounded-full"
          onClick={onContinue}
        >
          <Trans>Continue</Trans>
        </Button>
      ) : null}
    </div>
  );
}
