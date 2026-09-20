import { Trans, useLingui } from "@lingui/react/macro";
import {
  useMutation,
  useQueries,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { open as selectFiles } from "@tauri-apps/plugin-dialog";
import { type ReactNode, useEffect, useRef } from "react";

import { commands as importerCommands } from "@anlg/plugin-importer";
import {
  ArrowsClockwise,
  CaretDown,
  CircleNotch,
  DownloadSimple,
  PlugsConnected,
} from "@anlg/ui/components/icons";
import { Button } from "@anlg/ui/components/ui/button";
import { ButtonGroup } from "@anlg/ui/components/ui/button-group";
import {
  AppFloatingPanel,
  appFloatingMenuPanelClassName,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { useSquircleRef } from "@anlg/ui/hooks/use-squircle";
import { cn } from "@anlg/utils";

import {
  cancelConnectedImport,
  connectConnectedImport,
  connectedImportCredentialsQueryKey,
  connectedImportCredentialsQueryOptions,
  connectedImportSyncQueryKey,
  connectedImportSyncQueryOptions,
  disconnectConnectedImport,
  isDirectMeetingImport,
  isLocalConnectedImport,
} from "./connected-import";
import { detectImportSources } from "./detection";
import { loadFirefliesExportBundles } from "./fireflies-export";
import { providerIconOpticalClass, providerIconSrc } from "./icons";
import type {
  DetectedMeetingImportProvider,
  MeetingImportProvider,
} from "./providers";
import {
  EMPTY_MEETING_IMPORT_HISTORY,
  importFirefliesExportBundles,
  importMeetingFiles,
  useMeetingImportHistory,
} from "./queries";

const IMPORT_EXTENSIONS = [
  "csv",
  "json",
  "md",
  "markdown",
  "srt",
  "txt",
  "vtt",
];

function ImportSplitButtonGroup({ children }: { children: ReactNode }) {
  const ref = useSquircleRef<HTMLDivElement>();
  return (
    <div
      ref={ref}
      className={cn([
        "focus-within:ring-ring/50 w-fit overflow-hidden focus-within:ring-[3px]",
        "bg-primary",
      ])}
    >
      <ButtonGroup>{children}</ButtonGroup>
    </div>
  );
}

function ProviderIcon({
  provider,
}: {
  provider: DetectedMeetingImportProvider;
}) {
  const src = providerIconSrc(provider);
  if (src) {
    return (
      <img
        src={src}
        alt=""
        className={cn([
          "size-8 object-contain object-center",
          providerIconOpticalClass(provider),
        ])}
      />
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
  onNoSourcesDetected,
  secondaryAction,
}: {
  compact?: boolean;
  onContinue?: () => void;
  onNoSourcesDetected?: () => void;
  secondaryAction?: ReactNode;
}) {
  const { t } = useLingui();
  const queryClient = useQueryClient();
  const connectAbortController = useRef<AbortController | null>(null);
  const detectionQuery = useQuery({
    queryKey: ["meeting-import-sources"],
    queryFn: detectImportSources,
    refetchOnMount: "always",
  });
  const historyQuery = useMeetingImportHistory();
  const history = historyQuery.data ?? EMPTY_MEETING_IMPORT_HISTORY;
  const detectedProviders = detectionQuery.data ?? [];
  const connectedProviders = detectedProviders
    .filter((provider) => isDirectMeetingImport(provider))
    .sort((left, right) => left.name.localeCompare(right.name));
  const mcpProviders = connectedProviders.filter(isLocalConnectedImport);
  const fileProviders = detectedProviders
    .filter((provider) => !provider.directImport)
    .sort((left, right) => left.name.localeCompare(right.name));
  const displayedProviders = [...connectedProviders, ...fileProviders];
  const detectionSettled = !detectionQuery.isLoading && !detectionQuery.error;

  useEffect(() => {
    if (
      detectionQuery.isFetching ||
      detectionQuery.error ||
      detectionQuery.data?.length !== 0
    ) {
      return;
    }

    onNoSourcesDetected?.();
  }, [
    detectionQuery.data,
    detectionQuery.error,
    detectionQuery.isFetching,
    onNoSourcesDetected,
  ]);

  const connectedProvidersForQueries = mcpProviders;
  const credentialQueries = useQueries({
    queries: connectedProvidersForQueries.map((provider) =>
      connectedImportCredentialsQueryOptions(provider.id),
    ),
  });
  const syncQueries = useQueries({
    queries: connectedProvidersForQueries.map((provider, index) =>
      connectedImportSyncQueryOptions(
        provider,
        Boolean(credentialQueries[index]?.data),
      ),
    ),
  });
  const connectedProviderIndexes = new Map(
    connectedProvidersForQueries.map((provider, index) => [provider.id, index]),
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

  const firefliesExportImportMutation = useMutation({
    mutationFn: async () => {
      const selection = await selectFiles({
        title: t`Choose your Fireflies export folder`,
        multiple: false,
        directory: true,
      });
      if (!selection) return null;

      const bundles = await loadFirefliesExportBundles(selection);
      if (bundles.length === 0) {
        throw new Error(t`No Fireflies export found in that folder`);
      }
      return importFirefliesExportBundles(bundles);
    },
  });

  const connectMutation = useMutation({
    mutationFn: async (provider: MeetingImportProvider) => {
      const controller = new AbortController();
      connectAbortController.current = controller;
      try {
        return await connectConnectedImport(provider, controller.signal);
      } catch (error) {
        if (controller.signal.aborted) return null;
        throw error;
      } finally {
        if (connectAbortController.current === controller) {
          connectAbortController.current = null;
        }
      }
    },
    onSuccess: async (result) => {
      if (!result) return;
      queryClient.setQueryData(
        connectedImportCredentialsQueryKey(result.providerId),
        result,
      );
    },
  });

  const cancelConnectMutation = useMutation({
    mutationFn: cancelConnectedImport,
  });

  const disconnectMutation = useMutation({
    mutationFn: async (input: { providerId: string }) => {
      await disconnectConnectedImport(input.providerId);
    },
    onSuccess: async (_, input) => {
      queryClient.setQueryData(
        connectedImportCredentialsQueryKey(input.providerId),
        null,
      );
      await queryClient.cancelQueries({
        queryKey: connectedImportSyncQueryKey(input.providerId),
      });
      queryClient.removeQueries({
        queryKey: connectedImportSyncQueryKey(input.providerId),
      });
    },
  });

  const connectedError =
    credentialQueries.find((query) => query.error)?.error ??
    connectMutation.error ??
    cancelConnectMutation.error ??
    disconnectMutation.error ??
    firefliesExportImportMutation.error ??
    syncQueries.find((query) => query.error)?.error;
  const latestResult =
    fileImportMutation.data ??
    firefliesExportImportMutation.data ??
    syncQueries.find((query) => query.data)?.data?.result ??
    null;

  return (
    <div className={cn(["flex flex-col gap-4", compact && "max-w-3xl"])}>
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

      {displayedProviders.length > 0 || detectionSettled ? (
        <div className="border-border bg-card overflow-hidden rounded-2xl border">
          <div
            className={cn([
              "divide-border divide-y",
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
                const connectedProvider = isDirectMeetingImport(provider);
                const connectedIndex = connectedProviderIndexes.get(
                  provider.id,
                );
                const credentialsQuery =
                  connectedIndex === undefined
                    ? undefined
                    : credentialQueries[connectedIndex];
                const syncQuery =
                  connectedIndex === undefined
                    ? undefined
                    : syncQueries[connectedIndex];
                const connected = Boolean(credentialsQuery?.data);
                const checkingConnection = Boolean(credentialsQuery?.isPending);
                const connecting =
                  connectMutation.isPending &&
                  connectMutation.variables.id === provider.id;
                const connectionCancellationRequested =
                  connecting &&
                  Boolean(connectAbortController.current?.signal.aborted);
                const cancellingConnection =
                  cancelConnectMutation.isPending &&
                  cancelConnectMutation.variables === provider.id;
                const disconnecting =
                  disconnectMutation.isPending &&
                  disconnectMutation.variables?.providerId === provider.id;
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
                              Connected · New meetings are imported
                              automatically while BlackMushi is running.
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
                            Last import: {lastRun.imported} added,{" "}
                            {lastRun.matched} unchanged
                          </Trans>
                        </p>
                      ) : provider.access === "Export" ? (
                        <p className="text-muted-foreground mt-1 text-xs">
                          <Trans>Choose files exported from this app.</Trans>
                        </p>
                      ) : (
                        <p className="text-muted-foreground mt-1 text-xs">
                          <Trans>
                            Direct connection is not available yet. You can
                            still bring your history over with files.
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
                              onClick={() =>
                                disconnectMutation.mutate({
                                  providerId: provider.id,
                                })
                              }
                            >
                              <Trans>Disconnect</Trans>
                            </Button>
                          </>
                        ) : (
                          <ImportSplitButtonGroup>
                            <Button
                              type="button"
                              size="sm"
                              variant="default"
                              smoothCorners={false}
                              disabled={
                                checkingConnection ||
                                cancelConnectMutation.isPending ||
                                connectionCancellationRequested ||
                                (connectMutation.isPending && !connecting)
                              }
                              className={cn([
                                "rounded-none border-0 shadow-none",
                                "hover:bg-primary-foreground/10 bg-transparent",
                              ])}
                              onClick={() => {
                                if (connecting) {
                                  connectAbortController.current?.abort();
                                  cancelConnectMutation.mutate(provider.id);
                                  return;
                                }
                                connectMutation.mutate(provider);
                              }}
                            >
                              {checkingConnection ||
                              connecting ||
                              cancellingConnection ? (
                                <CircleNotch className="size-3.5 animate-spin" />
                              ) : (
                                <PlugsConnected className="size-3.5" />
                              )}
                              {connecting || cancellingConnection ? (
                                <Trans>Cancel</Trans>
                              ) : checkingConnection ? (
                                <Trans>Checking connection</Trans>
                              ) : (
                                <Trans>Connect & import</Trans>
                              )}
                            </Button>
                            <DropdownMenu>
                              <DropdownMenuTrigger asChild>
                                <Button
                                  type="button"
                                  size="sm"
                                  variant="default"
                                  smoothCorners={false}
                                  aria-label={t`Use files`}
                                  disabled={fileImportMutation.isPending}
                                  className={cn([
                                    "relative w-6 rounded-none border-0 px-0 shadow-none",
                                    "before:absolute before:inset-y-1.5 before:left-0 before:w-px",
                                    "hover:bg-primary-foreground/10 before:bg-primary-foreground/20 bg-transparent",
                                  ])}
                                >
                                  <CaretDown className="size-3.5" />
                                </Button>
                              </DropdownMenuTrigger>
                              <DropdownMenuContent
                                variant="app"
                                align="end"
                                className="w-40"
                              >
                                <AppFloatingPanel
                                  className={appFloatingMenuPanelClassName}
                                >
                                  <DropdownMenuItem
                                    onClick={() =>
                                      fileImportMutation.mutate(provider)
                                    }
                                  >
                                    <DownloadSimple />
                                    <Trans>Use files</Trans>
                                  </DropdownMenuItem>
                                </AppFloatingPanel>
                              </DropdownMenuContent>
                            </DropdownMenu>
                          </ImportSplitButtonGroup>
                        )}
                        {connected ? (
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
                        ) : null}
                        {provider.id === "fireflies" ? (
                          <Button
                            type="button"
                            size="sm"
                            variant="ghost"
                            disabled={firefliesExportImportMutation.isPending}
                            onClick={() =>
                              firefliesExportImportMutation.mutate()
                            }
                          >
                            {firefliesExportImportMutation.isPending ? (
                              <CircleNotch className="size-3.5 animate-spin" />
                            ) : (
                              <DownloadSimple className="size-3.5" />
                            )}
                            <Trans>Import export folder</Trans>
                          </Button>
                        ) : null}
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
        </div>
      ) : null}

      {secondaryAction || (onContinue && latestResult) ? (
        <div className="flex items-center gap-3">
          {onContinue && latestResult ? (
            <Button
              type="button"
              className="w-fit rounded-full"
              onClick={onContinue}
            >
              <Trans>Continue</Trans>
            </Button>
          ) : null}
          {secondaryAction}
        </div>
      ) : null}
    </div>
  );
}
