import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowsClockwise,
  CaretDown,
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
import { type ReactNode, useEffect, useRef } from "react";

import { commands as importerCommands } from "@anlg/plugin-importer";
import { Button } from "@anlg/ui/components/ui/button";
import { ButtonGroup } from "@anlg/ui/components/ui/button-group";
import {
  AppFloatingPanel,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { cn } from "@anlg/utils";

import {
  cancelConnectedImport,
  connectConnectedImport,
  connectNangoImport,
  connectedImportCredentialsQueryKey,
  connectedImportCredentialsQueryOptions,
  connectedImportSyncQueryKey,
  connectedImportSyncQueryOptions,
  disconnectConnectedImport,
  disconnectNangoImport,
  isDirectMeetingImport,
  isLocalConnectedImport,
  isNangoMeetingImport,
  nangoConnectionIsReady,
  nangoImportSyncQueryOptions,
} from "./connected-import";
import { detectImportSources } from "./detection";
import { providerIconOpticalClass, providerIconSrc } from "./icons";
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

import { useAuth } from "~/auth";
import { useConnections } from "~/auth/useConnections";
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
  const auth = useAuth();
  const queryClient = useQueryClient();
  const connectAbortController = useRef<AbortController | null>(null);
  const signedIn = Boolean(auth.session);
  const headers = auth.getHeaders();
  const connectionsQuery = useConnections(signedIn);
  const detectionQuery = useQuery({
    queryKey: ["meeting-import-sources"],
    queryFn: detectImportSources,
    refetchOnMount: "always",
  });
  useMountEffect(pauseCompetingApplicationTermination);
  const historyQuery = useMeetingImportHistory();
  const history = historyQuery.data ?? EMPTY_MEETING_IMPORT_HISTORY;
  const detectedProviders = detectionQuery.data ?? [];
  const connectedProviders = detectedProviders
    .filter((provider) => isDirectMeetingImport(provider))
    .sort((left, right) => left.name.localeCompare(right.name));
  const mcpProviders = connectedProviders.filter(isLocalConnectedImport);
  const nangoProviders = connectedProviders.filter(isNangoMeetingImport);
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
        signedIn && Boolean(credentialQueries[index]?.data),
      ),
    ),
  });
  const nangoSyncQueries = useQueries({
    queries: nangoProviders.map((provider) => {
      const connection = connectionsQuery.data?.find(
        (item) => item.integration_id === provider.nangoIntegrationId,
      );
      return nangoImportSyncQueryOptions(
        provider,
        connection?.connection_id,
        headers,
        signedIn && nangoConnectionIsReady(connection),
      );
    }),
  });
  const connectedProviderIndexes = new Map(
    connectedProvidersForQueries.map((provider, index) => [provider.id, index]),
  );
  const nangoProviderIndexes = new Map(
    nangoProviders.map((provider, index) => [provider.id, index]),
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
    mutationFn: async (provider: MeetingImportProvider) => {
      const controller = new AbortController();
      connectAbortController.current = controller;
      try {
        if (isNangoMeetingImport(provider)) {
          const sessionHeaders = auth.getHeaders();
          if (!sessionHeaders) {
            throw new Error("No authentication session is available");
          }
          return await connectNangoImport(
            provider,
            sessionHeaders,
            controller.signal,
          );
        }
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
      if ("connection_id" in result) {
        await queryClient.invalidateQueries({
          queryKey: ["integration-status"],
        });
        return;
      }
      queryClient.setQueryData(
        connectedImportCredentialsQueryKey(result.providerId),
        result,
      );
    },
  });

  const signInMutation = useMutation({
    mutationFn: () => auth.signIn(),
  });

  const cancelConnectMutation = useMutation({
    mutationFn: cancelConnectedImport,
  });

  const disconnectMutation = useMutation({
    mutationFn: async (input: {
      providerId: string;
      nangoIntegrationId?: string;
      connectionId?: string;
    }) => {
      if (input.nangoIntegrationId && input.connectionId) {
        await disconnectNangoImport(
          input.nangoIntegrationId,
          input.connectionId,
        );
        return;
      }
      await disconnectConnectedImport(input.providerId);
    },
    onSuccess: async (_, input) => {
      if (input.nangoIntegrationId) {
        await queryClient.invalidateQueries({
          queryKey: ["integration-status"],
        });
        await queryClient.cancelQueries({
          queryKey: connectedImportSyncQueryKey(input.providerId),
        });
        queryClient.removeQueries({
          queryKey: connectedImportSyncQueryKey(input.providerId),
        });
        return;
      }
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
    connectionsQuery.error ??
    signInMutation.error ??
    connectMutation.error ??
    cancelConnectMutation.error ??
    disconnectMutation.error ??
    syncQueries.find((query) => query.error)?.error ??
    nangoSyncQueries.find((query) => query.error)?.error;
  const latestResult =
    fileImportMutation.data ??
    syncQueries.find((query) => query.data)?.data?.result ??
    nangoSyncQueries.find((query) => query.data)?.data?.result ??
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
        .concat(nangoSyncQueries.flatMap((query) => query.data?.warnings ?? []))
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
                const nangoProvider = isNangoMeetingImport(provider);
                const connectedIndex = connectedProviderIndexes.get(
                  provider.id,
                );
                const nangoIndex = nangoProviderIndexes.get(provider.id);
                const credentialsQuery =
                  connectedIndex === undefined
                    ? undefined
                    : credentialQueries[connectedIndex];
                const nangoConnection = nangoProvider
                  ? connectionsQuery.data?.find(
                      (item) =>
                        item.integration_id === provider.nangoIntegrationId,
                    )
                  : undefined;
                const syncQuery = nangoProvider
                  ? nangoIndex === undefined
                    ? undefined
                    : nangoSyncQueries[nangoIndex]
                  : connectedIndex === undefined
                    ? undefined
                    : syncQueries[connectedIndex];
                const connected = nangoProvider
                  ? signedIn && nangoConnectionIsReady(nangoConnection)
                  : signedIn && Boolean(credentialsQuery?.data);
                const checkingConnection = nangoProvider
                  ? signedIn && connectionsQuery.isPending
                  : Boolean(credentialsQuery?.isPending);
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
                              automatically while Anarlog is running.
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
                                  nangoIntegrationId: nangoProvider
                                    ? provider.nangoIntegrationId
                                    : undefined,
                                  connectionId: nangoConnection?.connection_id,
                                })
                              }
                            >
                              <Trans>Disconnect</Trans>
                            </Button>
                          </>
                        ) : (
                          <ButtonGroup>
                            <Button
                              type="button"
                              size="sm"
                              variant={signedIn ? "default" : "outline"}
                              aria-label={
                                signedIn ? undefined : t`Sign in to connect`
                              }
                              disabled={
                                signedIn
                                  ? checkingConnection ||
                                    cancelConnectMutation.isPending ||
                                    connectionCancellationRequested ||
                                    (connectMutation.isPending && !connecting)
                                  : signInMutation.isPending
                              }
                              className={cn([
                                !signedIn &&
                                  "group/sign-in bg-muted hover:border-primary hover:bg-primary hover:text-primary-foreground focus-visible:border-primary focus-visible:bg-primary focus-visible:text-primary-foreground",
                              ])}
                              onClick={() => {
                                if (!signedIn) {
                                  signInMutation.mutate();
                                  return;
                                }
                                if (connecting) {
                                  connectAbortController.current?.abort();
                                  if (!nangoProvider) {
                                    cancelConnectMutation.mutate(provider.id);
                                  }
                                  return;
                                }
                                connectMutation.mutate(provider);
                              }}
                            >
                              {!signedIn ? (
                                signInMutation.isPending ? (
                                  <>
                                    <CircleNotch className="size-3.5 animate-spin" />
                                    <Trans>Opening…</Trans>
                                  </>
                                ) : (
                                  <span className="grid items-center overflow-hidden">
                                    <span className="invisible col-start-1 row-start-1">
                                      <Trans>Sign in to connect</Trans>
                                    </span>
                                    <span className="col-start-1 row-start-1 flex items-center justify-center gap-2 transition-transform duration-200 group-hover/sign-in:translate-y-full group-focus-visible/sign-in:translate-y-full">
                                      <PlugsConnected className="size-3.5" />
                                      <Trans>Connect & import</Trans>
                                    </span>
                                    <span className="col-start-1 row-start-1 flex -translate-y-full items-center justify-center transition-transform duration-200 group-hover/sign-in:translate-y-0 group-focus-visible/sign-in:translate-y-0">
                                      <Trans>Sign in to connect</Trans>
                                    </span>
                                  </span>
                                )
                              ) : checkingConnection ||
                                connecting ||
                                cancellingConnection ? (
                                <CircleNotch className="size-3.5 animate-spin" />
                              ) : (
                                <PlugsConnected className="size-3.5" />
                              )}
                              {!signedIn ? null : connecting ||
                                cancellingConnection ? (
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
                                  variant={signedIn ? "default" : "outline"}
                                  aria-label={t`Use files`}
                                  disabled={fileImportMutation.isPending}
                                  className={cn([
                                    "relative w-6 px-0 before:absolute before:inset-y-1.5 before:left-0 before:w-px",
                                    signedIn
                                      ? "before:bg-primary-foreground/20"
                                      : "before:bg-border",
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
                                <AppFloatingPanel className="p-1">
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
                          </ButtonGroup>
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
