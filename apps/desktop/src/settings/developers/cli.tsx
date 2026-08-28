import { t } from "@lingui/core/macro";
import { Trans } from "@lingui/react/macro";
import { CheckCircle, CircleNotch, Copy } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { colors, fonts, media, radii } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { SkillsRow } from "./skills";

import { commands, type EmbeddedCliStatus } from "~/types/tauri.gen";

const CLI_STATUS_QUERY_KEY = ["embedded-cli-status"] as const;

async function loadStatus() {
  const result = await commands.checkEmbeddedCli();
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

async function copyText(
  text: string,
  successMessage: string,
  fallbackErrorMessage: string,
) {
  try {
    await navigator.clipboard.writeText(text);
    sonnerToast.success(successMessage);
  } catch (error) {
    sonnerToast.error(
      error instanceof Error ? error.message : fallbackErrorMessage,
    );
  }
}

export function buildMcpConfiguration(command: string) {
  return JSON.stringify(
    {
      mcpServers: {
        anarlog: {
          command,
          args: ["mcp"],
        },
      },
    },
    null,
    2,
  );
}

export function getCliInstallNotification(status: EmbeddedCliStatus) {
  if (status.state === "installed") {
    return {
      type: "success" as const,
      message: t`${status.commandName} is ready to use`,
    };
  }

  return {
    type: "error" as const,
    message: status.details ?? t`${status.commandName} could not be installed`,
  };
}

export function CliSettingsSections() {
  const queryClient = useQueryClient();
  const statusQuery = useQuery({
    queryKey: CLI_STATUS_QUERY_KEY,
    queryFn: loadStatus,
  });
  const installMutation = useMutation({
    mutationFn: async () => {
      const result = await commands.installEmbeddedCli();
      if (result.status === "error") {
        throw new Error(result.error);
      }
      return result.data;
    },
    onSuccess: (status) => {
      queryClient.setQueryData(CLI_STATUS_QUERY_KEY, status);
      const notification = getCliInstallNotification(status);
      if (notification.type === "success") {
        sonnerToast.success(notification.message);
      } else {
        sonnerToast.error(notification.message);
      }
    },
    onError: (error) => sonnerToast.error(error.message),
  });

  const status = statusQuery.data;

  return (
    <CliSection
      status={status}
      isLoading={statusQuery.isPending}
      error={statusQuery.error}
      isInstalling={installMutation.isPending}
      onInstall={() => installMutation.mutate()}
    />
  );
}

function CliSection({
  status,
  isLoading,
  error,
  isInstalling,
  onInstall,
}: {
  status: EmbeddedCliStatus | undefined;
  isLoading: boolean;
  error: Error | null;
  isInstalling: boolean;
  onInstall: () => void;
}) {
  const canInstall =
    status?.supported === true &&
    status.state !== "resource_missing" &&
    status.state !== "conflict";
  const isInstalled = status?.state === "installed";

  return (
    <section {...stylex.props(styles.section)}>
      <h2 {...stylex.props(styles.heading)}>{t`CLI & MCP`}</h2>
      <div {...stylex.props(styles.content)}>
        <div {...stylex.props(styles.row)}>
          <div {...stylex.props(styles.identity)}>
            <h3 {...stylex.props(styles.title)}>
              <Trans>Anarlog CLI</Trans>
              {isInstalled && (
                <CheckCircle
                  aria-label={t`Installed`}
                  {...stylex.props(styles.installedIcon)}
                />
              )}
            </h3>
            <CliStatus status={status} isLoading={isLoading} error={error} />
          </div>
          <div {...stylex.props(styles.actions)}>
            <Button
              type="button"
              size="sm"
              disabled={!canInstall || isInstalling}
              onClick={onInstall}
            >
              {isInstalling ? (
                <CircleNotch {...stylex.props(styles.icon, styles.spinner)} />
              ) : isInstalled ? (
                t`Reinstall`
              ) : (
                t`Install`
              )}
            </Button>
          </div>
        </div>
        <McpRow status={status} />
        <SkillsRow />
      </div>
    </section>
  );
}

function CliStatus({
  status,
  isLoading,
  error,
}: {
  status: EmbeddedCliStatus | undefined;
  isLoading: boolean;
  error: Error | null;
}) {
  if (isLoading) {
    return (
      <span {...stylex.props(styles.loading)}>
        <CircleNotch {...stylex.props(styles.smallIcon, styles.spinner)} />
        {t`Checking…`}
      </span>
    );
  }

  if (error) {
    return (
      <p {...stylex.props(styles.error)}>
        {t`Could not check the CLI: ${error.message}`}
      </p>
    );
  }

  if (!status) {
    return null;
  }

  if (status.state === "installed") {
    return null;
  }

  const showDetails = ["conflict", "unsupported", "resource_missing"].includes(
    status.state,
  );

  return (
    <div {...stylex.props(styles.status)}>
      <span
        {...stylex.props(
          styles.statusDot,
          status.state === "conflict"
            ? styles.conflictDot
            : styles.unavailableDot,
        )}
      />
      <span {...stylex.props(styles.statusText)}>
        {showDetails ? (status.details ?? t`Unavailable`) : t`Not installed`}
      </span>
    </div>
  );
}

function McpRow({ status }: { status: EmbeddedCliStatus | undefined }) {
  const isInstalled = status?.state === "installed";
  const commandName = status?.commandName ?? "anarlog";
  const configuration = buildMcpConfiguration(
    isInstalled ? status.installPath : commandName,
  );

  return (
    <div {...stylex.props(styles.row)}>
      <div {...stylex.props(styles.identity)}>
        <h3 {...stylex.props(styles.subtitle)}>{t`MCP server`}</h3>
      </div>
      <div {...stylex.props(styles.actions)}>
        <Button
          type="button"
          variant="outline"
          size="sm"
          disabled={!isInstalled}
          onClick={() =>
            void copyText(
              configuration,
              t`MCP configuration copied`,
              t`Could not copy the MCP configuration`,
            )
          }
        >
          <Copy {...stylex.props(styles.icon)} />
          {t`Copy config`}
        </Button>
      </div>
    </div>
  );
}

const spin = stylex.keyframes({
  to: { transform: "rotate(360deg)" },
});

const styles = stylex.create({
  actions: {
    display: "flex",
    flexShrink: 0,
    gap: "0.5rem",
  },
  conflictDot: {
    backgroundColor: "#f59e0b",
  },
  content: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  error: {
    color: colors.destructive,
    fontSize: "0.75rem",
    lineHeight: "1rem",
    marginTop: "0.25rem",
  },
  heading: {
    fontFamily: fonts.sans,
    fontSize: "1.125rem",
    fontWeight: 600,
    lineHeight: "1.75rem",
  },
  icon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  identity: {
    minWidth: 0,
  },
  installedIcon: {
    color: "#059669",
    height: "0.875rem",
    width: "0.875rem",
  },
  loading: {
    alignItems: "center",
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.75rem",
    gap: "0.375rem",
    lineHeight: "1rem",
    marginTop: "0.25rem",
  },
  row: {
    alignItems: {
      default: null,
      [media.sm]: "center",
    },
    display: "flex",
    flexDirection: {
      default: "column",
      [media.sm]: "row",
    },
    gap: "1rem",
    justifyContent: {
      default: null,
      [media.sm]: "space-between",
    },
  },
  section: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  smallIcon: {
    height: "0.75rem",
    width: "0.75rem",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
  },
  status: {
    alignItems: "flex-start",
    display: "flex",
    fontSize: "0.75rem",
    gap: "0.375rem",
    lineHeight: "1rem",
    marginTop: "0.25rem",
  },
  statusDot: {
    borderRadius: radii.full,
    flexShrink: 0,
    height: "0.5rem",
    marginTop: "0.25rem",
    width: "0.5rem",
  },
  statusText: {
    overflowWrap: "anywhere",
    color: colors.mutedForeground,
  },
  subtitle: {
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
  },
  title: {
    alignItems: "center",
    display: "flex",
    fontSize: "0.875rem",
    fontWeight: 500,
    gap: "0.375rem",
    lineHeight: "1.25rem",
  },
  unavailableDot: {
    backgroundColor: `color-mix(in srgb, ${colors.mutedForeground} 50%, transparent)`,
  },
});
