import { Trans, useLingui } from "@lingui/react/macro";
import { CircleNotch, DownloadSimple } from "@phosphor-icons/react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { open as selectFiles } from "@tauri-apps/plugin-dialog";

import { commands as importerCommands } from "@anlg/plugin-importer";
import { Button } from "@anlg/ui/components/ui/button";
import { cn } from "@anlg/utils";

import { detectImportSources } from "./detection";
import type {
  DetectedMeetingImportProvider,
  MeetingImportProvider,
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

function ProviderIcon({
  provider,
}: {
  provider: DetectedMeetingImportProvider;
}) {
  if (provider.iconUrl) {
    return (
      <img src={provider.iconUrl} alt="" className="size-8 object-contain" />
    );
  }

  return (
    <span
      className="bg-muted text-muted-foreground flex size-8 items-center justify-center rounded-lg text-xs font-semibold"
      aria-hidden="true"
    >
      {provider.name.charAt(0)}
    </span>
  );
}

export function MeetingImportScreen({
  compact = false,
  onContinue,
}: {
  compact?: boolean;
  onContinue?: () => void;
}) {
  const { t } = useLingui();
  const detectionQuery = useQuery({
    queryKey: ["meeting-import-sources"],
    queryFn: detectImportSources,
    refetchOnMount: "always",
  });
  useMountEffect(pauseCompetingApplicationTermination);
  const historyQuery = useMeetingImportHistory();
  const history = historyQuery.data ?? EMPTY_MEETING_IMPORT_HISTORY;
  const detectedProviders = [...(detectionQuery.data ?? [])].sort(
    (left, right) => left.name.localeCompare(right.name),
  );

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

      {detectionQuery.data ? (
        <div
          className={cn([
            "border-border bg-card divide-border divide-y overflow-hidden rounded-2xl border",
            compact && "max-h-80 overflow-y-auto",
          ])}
        >
          {detectedProviders.length === 0 ? (
            <p className="text-muted-foreground px-4 py-6 text-center text-sm">
              <Trans>No apps found.</Trans>
            </p>
          ) : (
            detectedProviders.map((provider) => {
              const importing =
                importMutation.isPending &&
                importMutation.variables.id === provider.id;
              const lastRun = history.find(
                (run) => run.providerId === provider.id,
              );

              return (
                <div
                  key={provider.id}
                  className="flex min-h-16 items-center gap-3 px-4 py-3"
                >
                  <span className="flex size-8 shrink-0 items-center justify-center">
                    <ProviderIcon provider={provider} />
                  </span>
                  <div className="min-w-0 flex-1">
                    <span className="block truncate text-sm font-medium">
                      {provider.name}
                    </span>
                    {lastRun ? (
                      <p className="text-muted-foreground mt-1 text-xs">
                        <Trans>
                          Last import: {lastRun.imported} added,{" "}
                          {lastRun.matched} unchanged
                        </Trans>
                      </p>
                    ) : null}
                  </div>
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
            })
          )}
        </div>
      ) : null}

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
