import { t } from "@lingui/core/macro";
import { Trans } from "@lingui/react/macro";
import { CircleNotch, Copy, Key, LockSimple } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { colors, fonts, media, radii } from "@anlg/design-system/tokens.stylex";
import { Button } from "@anlg/ui/components/ui/button";
import { Input } from "@anlg/ui/components/ui/input";
import { Switch } from "@anlg/ui/components/ui/switch";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { ApiKeyRow } from "./api-key-row";
import { copyText } from "./clipboard";

import { useBillingAccess } from "~/auth/billing-context";
import {
  backfillCloudApiSnapshots,
  createCloudApiKey,
  getCloudApiSettings,
  listCloudApiKeys,
  revokeCloudApiKey,
  scheduleCloudApiBackfillRetry,
  setCloudApiEnabled,
  type CloudApiKey,
} from "~/cloud-api/client";
import { env } from "~/env";

const CLOUD_API_BASE_URL = new URL("/v1", env.VITE_API_URL).toString();
const CLOUD_MCP_URL = new URL("/mcp", env.VITE_API_URL).toString();
const CLOUD_API_SETTINGS_QUERY_KEY = ["cloud-api", "settings"] as const;
const CLOUD_API_KEYS_QUERY_KEY = ["cloud-api", "keys"] as const;

export function CloudApiSection() {
  const billing = useBillingAccess();
  const queryClient = useQueryClient();
  const settingsQuery = useQuery({
    queryKey: CLOUD_API_SETTINGS_QUERY_KEY,
    queryFn: getCloudApiSettings,
    enabled: billing.isReady && billing.isPro,
    retry: false,
  });
  const toggleMutation = useMutation({
    mutationFn: async (enabled: boolean) => {
      const settings = await setCloudApiEnabled(enabled);
      try {
        const uploaded = enabled ? await backfillCloudApiSnapshots() : 0;
        return { settings, uploaded, backfillFailed: false };
      } catch {
        scheduleCloudApiBackfillRetry();
        return { settings, uploaded: 0, backfillFailed: enabled };
      }
    },
    onSuccess: ({ settings, uploaded, backfillFailed }) => {
      queryClient.setQueryData(CLOUD_API_SETTINGS_QUERY_KEY, settings);
      void queryClient.invalidateQueries({
        queryKey: CLOUD_API_KEYS_QUERY_KEY,
      });
      if (backfillFailed) {
        sonnerToast.error(
          t`Cloud API enabled, but existing meetings could not be uploaded. Anarlog will retry.`,
        );
      } else if (settings.enabled) {
        sonnerToast.success(
          uploaded === 1
            ? t`Cloud API enabled — 1 meeting uploaded`
            : t`Cloud API enabled — ${uploaded} meetings uploaded`,
        );
      } else {
        sonnerToast.success(t`Cloud API disabled and readable copies deleted`);
      }
    },
    onError: (error) => sonnerToast.error(error.message),
  });
  const enabled = settingsQuery.data?.enabled === true;

  if (!billing.isReady) {
    return (
      <section {...stylex.props(styles.header)}>
        <CloudApiHeading />
        <CircleNotch
          aria-label={t`Loading Cloud API access`}
          {...stylex.props(styles.loadingIcon, styles.spinner)}
        />
      </section>
    );
  }

  if (!billing.isPro) {
    return (
      <section {...stylex.props(styles.lockedSection)}>
        <div {...stylex.props(styles.lockedCopy)}>
          <LockSimple {...stylex.props(styles.loadingIcon)} />
          <div>
            <h2 {...stylex.props(styles.heading)}>
              <Trans>Cloud API & Connectors</Trans>
            </h2>
            <p {...stylex.props(styles.description)}>
              <Trans>
                Access meetings remotely through the REST API and MCP connectors
                with Anarlog Pro.
              </Trans>
            </p>
          </div>
        </div>
        <Button
          type="button"
          size="sm"
          onClick={billing.upgradeToPro}
          disabled={billing.isUpgradingToPro}
        >
          {billing.isUpgradingToPro ? (
            <CircleNotch {...stylex.props(styles.mediumIcon, styles.spinner)} />
          ) : null}
          <Trans>Upgrade to Pro</Trans>
        </Button>
      </section>
    );
  }

  return (
    <section {...stylex.props(styles.section)}>
      <div {...stylex.props(styles.header)}>
        <CloudApiHeading error={settingsQuery.error?.message} />
        <Switch
          checked={enabled}
          aria-label={t`Enable Cloud API & Connectors`}
          disabled={
            settingsQuery.isPending ||
            settingsQuery.isError ||
            toggleMutation.isPending
          }
          onCheckedChange={(checked) => toggleMutation.mutate(checked)}
        />
      </div>

      {enabled && (
        <>
          <div {...stylex.props(styles.endpointGrid)}>
            <CloudEndpoint
              label={t`REST API`}
              value={CLOUD_API_BASE_URL}
              copyMessage={t`Cloud API URL copied`}
            />
            <CloudEndpoint
              label={t`Remote MCP`}
              value={CLOUD_MCP_URL}
              copyMessage={t`Remote MCP URL copied`}
            />
          </div>
          <CloudApiKeys />
        </>
      )}
    </section>
  );
}

function CloudApiHeading({ error }: { error?: string }) {
  return (
    <div {...stylex.props(styles.headingContainer)}>
      <h2 {...stylex.props(styles.heading)}>
        <Trans>Cloud API & Connectors</Trans>
      </h2>
      <p {...stylex.props(styles.mutedText)}>
        <Trans>
          Uploads meeting content for remote access while Anarlog is closed.
        </Trans>
      </p>
      {error ? <p {...stylex.props(styles.error)}>{error}</p> : null}
    </div>
  );
}

function CloudEndpoint({
  label,
  value,
  copyMessage,
}: {
  label: string;
  value: string;
  copyMessage: string;
}) {
  return (
    <div {...stylex.props(styles.headingContainer)}>
      <p {...stylex.props(styles.endpointLabel)}>{label}</p>
      <div {...stylex.props(styles.endpointRow)}>
        <code {...stylex.props(styles.code)}>{value}</code>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          sx={styles.copyButton}
          aria-label={t`Copy ${label} URL`}
          onClick={() => void copyText(value, copyMessage)}
        >
          <Copy {...stylex.props(styles.smallIcon)} />
        </Button>
      </div>
    </div>
  );
}

function CloudApiKeys() {
  const queryClient = useQueryClient();
  const keysQuery = useQuery({
    queryKey: CLOUD_API_KEYS_QUERY_KEY,
    queryFn: listCloudApiKeys,
  });
  const createMutation = useMutation({
    mutationFn: createCloudApiKey,
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: CLOUD_API_KEYS_QUERY_KEY }),
    onError: (error) => sonnerToast.error(error.message),
  });
  const revokeMutation = useMutation({
    mutationFn: revokeCloudApiKey,
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: CLOUD_API_KEYS_QUERY_KEY }),
    onError: (error) => sonnerToast.error(error.message),
  });
  const form = useForm({
    defaultValues: { name: "" },
    onSubmit: ({ value }) => {
      createMutation.mutate(value.name.trim() || t`Connector`);
      form.setFieldValue("name", "");
    },
  });
  const createdKey = createMutation.data;
  const keys = keysQuery.data ?? [];

  return (
    <div>
      <div {...stylex.props(styles.keysHeader)}>
        <Key {...stylex.props(styles.mutedIcon)} />
        <h4 {...stylex.props(styles.subheading)}>
          <Trans>Cloud API keys</Trans>
        </h4>
      </div>
      <form
        {...stylex.props(styles.form)}
        onSubmit={(event) => {
          event.preventDefault();
          event.stopPropagation();
          void form.handleSubmit();
        }}
      >
        <form.Field name="name">
          {(field) => (
            <Input
              sx={styles.keyInput}
              placeholder={t`Key name (e.g. Claude Code)`}
              value={field.state.value}
              onChange={(event) => field.handleChange(event.target.value)}
            />
          )}
        </form.Field>
        <Button
          type="submit"
          size="sm"
          variant="outline"
          disabled={createMutation.isPending}
        >
          <Trans>Create key</Trans>
        </Button>
      </form>

      {createdKey && (
        <div {...stylex.props(styles.createdCard)}>
          <p {...stylex.props(styles.createdDescription)}>
            <Trans>Copy this key now — it is only shown once.</Trans>
          </p>
          <div {...stylex.props(styles.secretRow)}>
            <code {...stylex.props(styles.code)}>{createdKey.key}</code>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              sx={styles.copyKeyButton}
              onClick={async () => {
                if (await copyText(createdKey.key, t`Cloud API key copied`)) {
                  createMutation.reset();
                }
              }}
            >
              <Copy {...stylex.props(styles.smallIcon)} />
              <Trans>Copy</Trans>
            </Button>
          </div>
        </div>
      )}

      {keys.length > 0 && (
        <ul {...stylex.props(styles.keysList)}>
          {keys.map((key) => (
            <CloudApiKeyRow
              key={key.id}
              apiKey={key}
              onRevoke={() => revokeMutation.mutate(key.id)}
            />
          ))}
        </ul>
      )}
    </div>
  );
}

function CloudApiKeyRow({
  apiKey,
  onRevoke,
}: {
  apiKey: CloudApiKey;
  onRevoke: () => void;
}) {
  return <ApiKeyRow apiKey={apiKey} onRevoke={onRevoke} />;
}

const spin = stylex.keyframes({
  to: { transform: "rotate(360deg)" },
});

const styles = stylex.create({
  code: {
    backgroundColor: colors.muted,
    borderRadius: radii.md,
    display: {
      default: null,
      "::-webkit-scrollbar": "none",
    },
    fontSize: "0.75rem",
    lineHeight: "1rem",
    minWidth: 0,
    overflowX: "auto",
    paddingBlock: "0.125rem",
    paddingInline: "0.375rem",
    scrollbarWidth: "none",
  },
  copyButton: {
    flexShrink: 0,
    height: "1.75rem",
    padding: 0,
    width: "1.75rem",
  },
  copyKeyButton: {
    flexShrink: 0,
    height: "1.75rem",
  },
  createdCard: {
    backgroundColor: `color-mix(in srgb, ${colors.muted} 30%, transparent)`,
    borderColor: colors.border,
    borderRadius: radii.xl,
    borderStyle: "solid",
    borderWidth: "1px",
    marginTop: "0.75rem",
    padding: "0.75rem",
  },
  createdDescription: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  description: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1.25rem",
    marginTop: "0.25rem",
  },
  endpointGrid: {
    display: "grid",
    gap: "0.75rem",
    gridTemplateColumns: {
      default: null,
      [media.sm]: "repeat(2, minmax(0, 1fr))",
    },
  },
  endpointLabel: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    fontWeight: 500,
    lineHeight: "1rem",
  },
  endpointRow: {
    alignItems: "center",
    display: "flex",
    gap: "0.25rem",
    marginTop: "0.25rem",
  },
  error: {
    color: colors.destructive,
    fontSize: "0.75rem",
    lineHeight: "1rem",
    marginTop: "0.5rem",
  },
  form: {
    display: "flex",
    gap: "0.5rem",
  },
  header: {
    alignItems: "flex-start",
    display: "flex",
    gap: "1rem",
    justifyContent: "space-between",
  },
  heading: {
    fontFamily: fonts.sans,
    fontSize: "1.125rem",
    fontWeight: 600,
    lineHeight: "1.75rem",
  },
  headingContainer: {
    minWidth: 0,
  },
  keyInput: {
    fontSize: "0.875rem",
    height: "2rem",
    lineHeight: "1.25rem",
    maxWidth: "16rem",
  },
  keysHeader: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    marginBottom: "0.75rem",
  },
  keysList: {
    display: "flex",
    flexDirection: "column",
    gap: "0.375rem",
    marginTop: "0.75rem",
  },
  loadingIcon: {
    color: colors.mutedForeground,
    flexShrink: 0,
    height: "1rem",
    marginTop: "0.25rem",
    width: "1rem",
  },
  lockedCopy: {
    display: "flex",
    gap: "0.75rem",
  },
  lockedSection: {
    alignItems: "flex-start",
    display: "flex",
    gap: "1.5rem",
    justifyContent: "space-between",
  },
  mediumIcon: {
    height: "1rem",
    width: "1rem",
  },
  mutedIcon: {
    color: colors.mutedForeground,
    height: "1rem",
    width: "1rem",
  },
  mutedText: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
    marginTop: "0.25rem",
  },
  section: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  secretRow: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    marginTop: "0.5rem",
  },
  smallIcon: {
    height: "0.875rem",
    width: "0.875rem",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
  },
  subheading: {
    fontSize: "0.875rem",
    fontWeight: 500,
    lineHeight: "1.25rem",
  },
});
