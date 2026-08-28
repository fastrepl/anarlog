import { Trans, useLingui } from "@lingui/react/macro";
import {
  ArrowsClockwise,
  CaretDown,
  CircleNotch,
  DownloadSimple,
  PlugsConnected,
} from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import {
  useMutation,
  useQueries,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { open as selectFiles } from "@tauri-apps/plugin-dialog";
import { type ReactNode, useEffect, useRef } from "react";

import { colors, radii } from "@anlg/design-system/tokens.stylex";
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
import { providerIconOpticalScale, providerIconSrc } from "./icons";
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
        {...stylex.props(
          styles.providerIcon,
          styles.providerIconScale(providerIconOpticalScale(provider)),
        )}
      />
    );
  }

  return (
    <span {...stylex.props(styles.providerFallback)} aria-hidden="true">
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
    <div {...stylex.props(styles.root, compact && styles.compact)}>
      {detectionQuery.isLoading ? (
        <p {...stylex.props(styles.loading)}>
          <CircleNotch {...stylex.props(styles.smallIcon, styles.spinning)} />
          <Trans>Checking installed meeting assistants…</Trans>
        </p>
      ) : detectionQuery.error ? (
        <p {...stylex.props(styles.errorSmall)}>
          {detectionQuery.error.message}
        </p>
      ) : null}

      {fileImportMutation.error ? (
        <p {...stylex.props(styles.error)}>
          {fileImportMutation.error.message}
        </p>
      ) : null}
      {connectedError ? (
        <p {...stylex.props(styles.error)}>{connectedError.message}</p>
      ) : null}
      {latestResult ? (
        <div {...stylex.props(styles.result)}>
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
          <p key={warning} {...stylex.props(styles.mutedSmall)}>
            {warning}
          </p>
        ))}

      {displayedProviders.length > 0 || detectionSettled ? (
        <div {...stylex.props(styles.providersCard)}>
          <div {...stylex.props(compact && styles.providerListCompact)}>
            {displayedProviders.length === 0 ? (
              <p {...stylex.props(styles.empty)}>
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
                  <div key={provider.id} {...stylex.props(styles.providerRow)}>
                    <span {...stylex.props(styles.providerIconSlot)}>
                      <ProviderIcon provider={provider} />
                    </span>
                    <div {...stylex.props(styles.providerCopy)}>
                      <span {...stylex.props(styles.providerName)}>
                        {provider.name}
                      </span>
                      {connectedProvider ? (
                        <p {...stylex.props(styles.providerDescription)}>
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
                        <p {...stylex.props(styles.providerDescription)}>
                          <Trans>
                            Last import: {lastRun.imported} added,{" "}
                            {lastRun.matched} unchanged
                          </Trans>
                        </p>
                      ) : provider.access === "Export" ? (
                        <p {...stylex.props(styles.providerDescription)}>
                          <Trans>Choose files exported from this app.</Trans>
                        </p>
                      ) : (
                        <p {...stylex.props(styles.providerDescription)}>
                          <Trans>
                            Direct connection is not available yet. You can
                            still bring your history over with files.
                          </Trans>
                        </p>
                      )}
                    </div>
                    {connectedProvider ? (
                      <div {...stylex.props(styles.providerActions)}>
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
                                <CircleNotch
                                  {...stylex.props(
                                    styles.smallIcon,
                                    styles.spinning,
                                  )}
                                />
                              ) : (
                                <ArrowsClockwise
                                  {...stylex.props(styles.smallIcon)}
                                />
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
                              data-sign-in={!signedIn || undefined}
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
                              sx={!signedIn ? styles.signInButton : undefined}
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
                                    <CircleNotch
                                      {...stylex.props(
                                        styles.smallIcon,
                                        styles.spinning,
                                      )}
                                    />
                                    <Trans>Opening…</Trans>
                                  </>
                                ) : (
                                  <span {...stylex.props(styles.signInLabels)}>
                                    <span
                                      {...stylex.props(styles.signInMeasure)}
                                    >
                                      <Trans>Sign in to connect</Trans>
                                    </span>
                                    <span
                                      {...stylex.props(styles.connectLabel)}
                                    >
                                      <PlugsConnected
                                        {...stylex.props(styles.smallIcon)}
                                      />
                                      <Trans>Connect & import</Trans>
                                    </span>
                                    <span {...stylex.props(styles.signInLabel)}>
                                      <Trans>Sign in to connect</Trans>
                                    </span>
                                  </span>
                                )
                              ) : checkingConnection ||
                                connecting ||
                                cancellingConnection ? (
                                <CircleNotch
                                  {...stylex.props(
                                    styles.smallIcon,
                                    styles.spinning,
                                  )}
                                />
                              ) : (
                                <PlugsConnected
                                  {...stylex.props(styles.smallIcon)}
                                />
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
                                  sx={styles.fileMenuButton}
                                >
                                  <span
                                    aria-hidden="true"
                                    {...stylex.props(
                                      styles.fileMenuDivider,
                                      signedIn
                                        ? styles.fileMenuDividerSignedIn
                                        : styles.fileMenuDividerSignedOut,
                                    )}
                                  />
                                  <CaretDown
                                    {...stylex.props(styles.smallIcon)}
                                  />
                                </Button>
                              </DropdownMenuTrigger>
                              <DropdownMenuContent
                                variant="app"
                                align="end"
                                sx={styles.fileMenu}
                              >
                                <AppFloatingPanel sx={styles.fileMenuPanel}>
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
                            <DownloadSimple
                              {...stylex.props(styles.smallIcon)}
                            />
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
                          <CircleNotch
                            {...stylex.props(styles.smallIcon, styles.spinning)}
                          />
                        ) : (
                          <DownloadSimple {...stylex.props(styles.smallIcon)} />
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
        <div {...stylex.props(styles.secondaryActions)}>
          {onContinue && latestResult ? (
            <Button
              type="button"
              sx={styles.continueButton}
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

const spin = stylex.keyframes({
  to: {
    transform: "rotate(360deg)",
  },
});

const styles = stylex.create({
  compact: {
    maxWidth: "48rem",
  },
  connectLabel: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    gridColumnStart: "1",
    gridRowStart: "1",
    justifyContent: "center",
    transform: {
      default: "translateY(0)",
      ":is([data-sign-in]:focus-visible *)": "translateY(100%)",
      ":is([data-sign-in]:hover *)": "translateY(100%)",
    },
    transitionDuration: "200ms",
    transitionProperty: "transform",
  },
  continueButton: {
    borderRadius: radii.full,
    width: "fit-content",
  },
  empty: {
    color: colors.mutedForeground,
    fontSize: "0.875rem",
    paddingBlock: "1.5rem",
    paddingInline: "1rem",
    textAlign: "center",
  },
  error: {
    color: colors.destructive,
    fontSize: "0.875rem",
  },
  errorSmall: {
    color: colors.destructive,
    fontSize: "0.75rem",
  },
  fileMenu: {
    width: "10rem",
  },
  fileMenuButton: {
    paddingInline: 0,
    position: "relative",
    width: "1.5rem",
  },
  fileMenuDivider: {
    bottom: "0.375rem",
    left: 0,
    position: "absolute",
    top: "0.375rem",
    width: "1px",
  },
  fileMenuDividerSignedIn: {
    backgroundColor: `color-mix(in srgb, ${colors.primaryForeground} 20%, transparent)`,
  },
  fileMenuDividerSignedOut: {
    backgroundColor: colors.border,
  },
  fileMenuPanel: {
    padding: "0.25rem",
  },
  loading: {
    alignItems: "center",
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.75rem",
    gap: "0.5rem",
  },
  mutedSmall: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
  },
  providerActions: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    gap: "0.25rem",
  },
  providerCopy: {
    flex: "1",
    minWidth: 0,
  },
  providerDescription: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    marginTop: "0.25rem",
  },
  providerFallback: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderRadius: radii.lg,
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.75rem",
    fontWeight: 600,
    height: "2rem",
    justifyContent: "center",
    width: "2rem",
  },
  providerIcon: {
    height: "2rem",
    objectFit: "contain",
    objectPosition: "center",
    width: "2rem",
  },
  providerIconScale: (scale: number | undefined) => ({
    transform: scale === undefined ? null : `scale(${scale})`,
  }),
  providerIconSlot: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    height: "2rem",
    justifyContent: "center",
    width: "2rem",
  },
  providerListCompact: {
    maxHeight: "20rem",
    overflowY: "auto",
  },
  providerName: {
    display: "block",
    fontSize: "0.875rem",
    fontWeight: 500,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  providerRow: {
    alignItems: "center",
    borderTopColor: colors.border,
    borderTopStyle: "solid",
    borderTopWidth: {
      default: "1px",
      ":first-child": 0,
    },
    display: "flex",
    gap: "0.75rem",
    minHeight: "4rem",
    paddingBlock: "0.75rem",
    paddingInline: "1rem",
  },
  providersCard: {
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: "1rem",
    borderStyle: "solid",
    borderWidth: "1px",
    overflow: "hidden",
  },
  result: {
    backgroundColor: colors.card,
    borderColor: colors.border,
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    fontSize: "0.875rem",
    paddingBlock: "0.75rem",
    paddingInline: "1rem",
  },
  root: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  secondaryActions: {
    alignItems: "center",
    display: "flex",
    gap: "0.75rem",
  },
  signInButton: {
    backgroundColor: {
      default: colors.muted,
      ":focus-visible": colors.primary,
      ":hover": colors.primary,
    },
    borderColor: {
      default: null,
      ":focus-visible": colors.primary,
      ":hover": colors.primary,
    },
    color: {
      default: null,
      ":focus-visible": colors.primaryForeground,
      ":hover": colors.primaryForeground,
    },
  },
  signInLabel: {
    alignItems: "center",
    display: "flex",
    gridColumnStart: "1",
    gridRowStart: "1",
    justifyContent: "center",
    transform: {
      default: "translateY(-100%)",
      ":is([data-sign-in]:focus-visible *)": "translateY(0)",
      ":is([data-sign-in]:hover *)": "translateY(0)",
    },
    transitionDuration: "200ms",
    transitionProperty: "transform",
  },
  signInLabels: {
    alignItems: "center",
    display: "grid",
    overflow: "hidden",
  },
  signInMeasure: {
    gridColumnStart: "1",
    gridRowStart: "1",
    visibility: "hidden",
  },
  smallIcon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  spinning: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
  },
});

export { styles as meetingImportScreenStyles };
