import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowsClockwise,
  CircleNotch,
  DownloadSimple,
  PlugsConnected,
} from "@phosphor-icons/react";
import {
  useMutation,
  useQueries,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { open as selectFiles } from "@tauri-apps/plugin-dialog";

import { commands as importerCommands } from "@anlg/plugin-importer";
import { Button } from "@anlg/ui/components/ui/button";
import { cn } from "@anlg/utils";

import {
  connectConnectedImport,
  connectedImportCredentialsQueryKey,
  connectedImportCredentialsQueryOptions,
  connectedImportSyncQueryKey,
  connectedImportSyncQueryOptions,
  disconnectConnectedImport,
} from "./connected-import";
import { detectImportSources } from "./detection";
import type {
  DetectedMeetingImportProvider,
  MeetingImportProvider,
} from "./providers";
import { MEETING_IMPORT_PROVIDERS } from "./providers";
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
  provider: MeetingImportProvider &
    Partial<Pick<DetectedMeetingImportProvider, "iconUrl">>;
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
  const queryClient = useQueryClient();
  const detectionQuery = useQuery({
    queryKey: ["meeting-import-sources"],
    queryFn: detectImportSources,
    refetchOnMount: "always",
  });
  useMountEffect(pauseCompetingApplicationTermination);
  const historyQuery = useMeetingImportHistory();
  const history = historyQuery.data ?? EMPTY_MEETING_IMPORT_HISTORY;
  const detectedProviders = detectionQuery.data ?? [];
  const detectedProvidersById = new Map(
    detectedProviders.map((provider) => [provider.id, provider]),
  );
  const directProviders = MEETING_IMPORT_PROVIDERS.filter(
    (provider) => provider.directImport === "mcp-oauth",
  )
    .map((provider) => detectedProvidersById.get(provider.id) ?? provider)
    .sort(
      (left, right) =>
        Number(detectedProvidersById.has(right.id)) -
        Number(detectedProvidersById.has(left.id)),
    );
  const fileProviders = detectedProviders
    .filter((provider) => !provider.directImport)
    .sort((left, right) => left.name.localeCompare(right.name));
  const displayedProviders = [...directProviders, ...fileProviders];
  const connectedProviders = directProviders;
  const credentialQueries = useQueries({
    queries: connectedProviders.map((provider) =>
      connectedImportCredentialsQueryOptions(provider.id),
    ),
  });
  const syncQueries = useQueries({
    queries: connectedProviders.map((provider, index) =>
      connectedImportSyncQueryOptions(
        provider,
        Boolean(credentialQueries[index]?.data),
      ),
    ),
  });
  const connectedProviderIndexes = new Map(
    connectedProviders.map((provider, index) => [provider.id, index]),
  );

  const fileImportMutation = useMutation({
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

  const connectMutation = useMutation({
    mutationFn: connectConnectedImport,
    onSuccess: (credentials) => {
      queryClient.setQueryData(
        connectedImportCredentialsQueryKey(credentials.providerId),
        credentials,
      );
    },
  });

  const disconnectMutation = useMutation({
    mutationFn: (providerId: string) => disconnectConnectedImport(providerId),
    onSuccess: async (_, providerId) => {
      queryClient.setQueryData(
        connectedImportCredentialsQueryKey(providerId),
        null,
      );
      await queryClient.cancelQueries({
        queryKey: connectedImportSyncQueryKey(providerId),
      });
      queryClient.removeQueries({
        queryKey: connectedImportSyncQueryKey(providerId),
      });
    },
  });

  const connectedError =
    credentialQueries.find((query) => query.error)?.error ??
    connectMutation.error ??
    disconnectMutation.error ??
    syncQueries.find((query) => query.error)?.error;
  const latestResult =
    fileImportMutation.data ??
    syncQueries.find((query) => query.data)?.data?.result ??
    null;

  return (
    <div className={cn(["flex flex-col gap-4", compact && "max-w-3xl"])}>
      <div className="flex flex-col gap-1">
        <p className="text-muted-foreground text-sm">
          <Trans>
            Bring your meeting history with you. Connect supported assistants to
            keep new meetings coming into Anarlog, or use export files as a
            fallback.
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

      {fileImportMutation.error ? (
        <p className="text-destructive text-sm">
          {fileImportMutation.error.message}
        </p>
      ) : null}
      {connectedError ? (
        <p className="text-destructive text-sm">{connectedError.message}</p>
      ) : null}
      {latestResult ? (
        <div className="border-border bg-card rounded-xl border px-4 py-3 text-sm">
          {latestResult.imported > 0 ? (
            <Trans>
              Brought in {latestResult.imported} new meetings.{" "}
              {latestResult.matched} were already here.
            </Trans>
          ) : latestResult.errors > 0 || latestResult.conflicts > 0 ? (
            <Trans>
              Nothing new was imported. {latestResult.conflicts} meetings need
              review and {latestResult.errors} could not be imported.
            </Trans>
          ) : (
            <Trans>Everything is already here.</Trans>
          )}
        </div>
      ) : null}
      {syncQueries
        .flatMap((query) => query.data?.warnings ?? [])
        .map((warning) => (
          <p key={warning} className="text-muted-foreground text-xs">
            {warning}
          </p>
        ))}

      <div
        className={cn([
          "border-border bg-card divide-border divide-y overflow-hidden rounded-2xl border",
          compact && "max-h-80 overflow-y-auto",
        ])}
      >
        {displayedProviders.length === 0 ? (
          <p className="text-muted-foreground px-4 py-6 text-center text-sm">
            <Trans>No apps found.</Trans>
          </p>
        ) : (
          displayedProviders.map((provider) => {
            const importing =
              fileImportMutation.isPending &&
              fileImportMutation.variables.id === provider.id;
            const connectedProvider = provider.directImport === "mcp-oauth";
            const connectedIndex = connectedProviderIndexes.get(provider.id);
            const credentialsQuery =
              connectedIndex === undefined
                ? undefined
                : credentialQueries[connectedIndex];
            const syncQuery =
              connectedIndex === undefined
                ? undefined
                : syncQueries[connectedIndex];
            const connected = Boolean(credentialsQuery?.data);
            const connecting =
              connectMutation.isPending &&
              connectMutation.variables.id === provider.id;
            const disconnecting =
              disconnectMutation.isPending &&
              disconnectMutation.variables === provider.id;
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
                  {connectedProvider ? (
                    <p className="text-muted-foreground mt-1 text-xs">
                      {connected ? (
                        <Trans>
                          Connected · New meetings are imported automatically
                          while Anarlog is running.
                        </Trans>
                      ) : (
                        <Trans>
                          Connect once to bring over your {provider.name}{" "}
                          history and keep new meetings coming in while you
                          switch.
                        </Trans>
                      )}
                    </p>
                  ) : lastRun ? (
                    <p className="text-muted-foreground mt-1 text-xs">
                      <Trans>
                        Last import: {lastRun.imported} added, {lastRun.matched}{" "}
                        unchanged
                      </Trans>
                    </p>
                  ) : provider.access === "Export" ? (
                    <p className="text-muted-foreground mt-1 text-xs">
                      <Trans>Choose files exported from this app.</Trans>
                    </p>
                  ) : (
                    <p className="text-muted-foreground mt-1 text-xs">
                      <Trans>
                        Direct connection is not available yet. You can still
                        bring your history over with files.
                      </Trans>
                    </p>
                  )}
                </div>
                {connectedProvider ? (
                  <div className="flex shrink-0 items-center gap-1">
                    {connected ? (
                      <>
                        <Button
                          type="button"
                          size="sm"
                          variant="outline"
                          disabled={syncQuery?.isFetching}
                          onClick={() => void syncQuery?.refetch()}
                        >
                          {syncQuery?.isFetching ? (
                            <CircleNotch className="size-3.5 animate-spin" />
                          ) : (
                            <ArrowsClockwise className="size-3.5" />
                          )}
                          <Trans>Sync now</Trans>
                        </Button>
                        <Button
                          type="button"
                          size="sm"
                          variant="ghost"
                          disabled={syncQuery?.isFetching || disconnecting}
                          onClick={() => disconnectMutation.mutate(provider.id)}
                        >
                          <Trans>Disconnect</Trans>
                        </Button>
                      </>
                    ) : (
                      <Button
                        type="button"
                        size="sm"
                        disabled={
                          credentialsQuery?.isPending ||
                          connectMutation.isPending
                        }
                        onClick={() => connectMutation.mutate(provider)}
                      >
                        {credentialsQuery?.isPending || connecting ? (
                          <CircleNotch className="size-3.5 animate-spin" />
                        ) : (
                          <PlugsConnected className="size-3.5" />
                        )}
                        {credentialsQuery?.isPending ? (
                          <Trans>Checking connection</Trans>
                        ) : (
                          <Trans>Connect & import</Trans>
                        )}
                      </Button>
                    )}
                    <Button
                      type="button"
                      size="sm"
                      variant="ghost"
                      disabled={fileImportMutation.isPending}
                      onClick={() => fileImportMutation.mutate(provider)}
                    >
                      <DownloadSimple className="size-3.5" />
                      <Trans>Use files</Trans>
                    </Button>
                  </div>
                ) : (
                  <Button
                    type="button"
                    size="sm"
                    variant="outline"
                    disabled={fileImportMutation.isPending}
                    onClick={() => fileImportMutation.mutate(provider)}
                  >
                    {importing ? (
                      <CircleNotch className="size-3.5 animate-spin" />
                    ) : (
                      <DownloadSimple className="size-3.5" />
                    )}
                    <Trans>Choose files</Trans>
                  </Button>
                )}
              </div>
            );
          })
        )}
      </div>

      {onContinue && latestResult ? (
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
