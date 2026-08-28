import { t } from "@lingui/core/macro";
import { Trans } from "@lingui/react/macro";
import { Copy, Trash } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { colors, fonts, radii } from "@anlg/design-system/tokens.stylex";
import {
  commands as webhookCommands,
  type WebhookInfo,
} from "@anlg/plugin-local-api";
import { Button } from "@anlg/ui/components/ui/button";
import { Input } from "@anlg/ui/components/ui/input";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { copyText } from "./clipboard";

const WEBHOOKS_QUERY_KEY = ["webhooks"] as const;

async function unwrap<T>(
  promise: Promise<
    { status: "ok"; data: T } | { status: "error"; error: string }
  >,
): Promise<T> {
  const result = await promise;
  if (result.status === "error") {
    throw new Error(result.error);
  }
  return result.data;
}

export function WebhooksSection() {
  const queryClient = useQueryClient();
  const webhooksQuery = useQuery({
    queryKey: WEBHOOKS_QUERY_KEY,
    queryFn: () => unwrap(webhookCommands.listWebhooks()),
  });
  const createMutation = useMutation({
    mutationFn: (url: string) => unwrap(webhookCommands.createWebhook(url, [])),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: WEBHOOKS_QUERY_KEY }),
    onError: (error) => sonnerToast.error(error.message),
  });
  const deleteMutation = useMutation({
    mutationFn: (id: string) => unwrap(webhookCommands.deleteWebhook(id)),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: WEBHOOKS_QUERY_KEY }),
    onError: (error) => sonnerToast.error(error.message),
  });
  const setActiveMutation = useMutation({
    mutationFn: ({ id, active }: { id: string; active: boolean }) =>
      unwrap(webhookCommands.setWebhookActive(id, active)),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: WEBHOOKS_QUERY_KEY }),
    onError: (error) => sonnerToast.error(error.message),
  });
  const testMutation = useMutation({
    mutationFn: (id: string) => unwrap(webhookCommands.testWebhook(id)),
    onSuccess: (delivery) => {
      if (delivery.delivered) {
        sonnerToast.success(t`Test delivered (${delivery.status})`);
      } else {
        sonnerToast.error(t`Test failed (${delivery.status})`);
      }
      void queryClient.invalidateQueries({ queryKey: WEBHOOKS_QUERY_KEY });
    },
    onError: (error) => sonnerToast.error(error.message),
  });

  const form = useForm({
    defaultValues: { url: "" },
    onSubmit: ({ value }) => {
      const url = value.url.trim();
      if (!url) {
        return;
      }
      createMutation.mutate(url);
      form.setFieldValue("url", "");
    },
  });

  const createdWebhook = createMutation.data;
  const webhooks = webhooksQuery.data ?? [];

  return (
    <section {...stylex.props(styles.section)}>
      <h2 {...stylex.props(styles.heading)}>{t`Webhooks`}</h2>
      <div>
        <form
          {...stylex.props(styles.form)}
          onSubmit={(event) => {
            event.preventDefault();
            event.stopPropagation();
            void form.handleSubmit();
          }}
        >
          <form.Field name="url">
            {(field) => (
              <Input
                sx={styles.input}
                placeholder="https://example.com/webhooks/anarlog"
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
            <Trans>Add webhook</Trans>
          </Button>
        </form>

        {createdWebhook && (
          <div {...stylex.props(styles.createdCard)}>
            <p {...stylex.props(styles.mutedText)}>
              <Trans>
                Copy this signing secret now — it is only shown once.
              </Trans>
            </p>
            <div {...stylex.props(styles.secretRow)}>
              <code {...stylex.props(styles.code)}>
                {createdWebhook.secret}
              </code>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                sx={styles.copyButton}
                onClick={async () => {
                  if (
                    await copyText(
                      createdWebhook.secret,
                      t`Signing secret copied`,
                    )
                  ) {
                    createMutation.reset();
                  }
                }}
              >
                <Copy {...stylex.props(styles.icon)} />
                <Trans>Copy</Trans>
              </Button>
            </div>
          </div>
        )}

        {webhooks.length > 0 && (
          <ul {...stylex.props(styles.list)}>
            {webhooks.map((webhook) => (
              <WebhookRow
                key={webhook.id}
                webhook={webhook}
                onTest={() => testMutation.mutate(webhook.id)}
                onDelete={() => deleteMutation.mutate(webhook.id)}
                onToggleActive={() =>
                  setActiveMutation.mutate({
                    id: webhook.id,
                    active: !webhook.active,
                  })
                }
                isTesting={testMutation.isPending}
              />
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}

function WebhookRow({
  webhook,
  onTest,
  onDelete,
  onToggleActive,
  isTesting,
}: {
  webhook: WebhookInfo;
  onTest: () => void;
  onDelete: () => void;
  onToggleActive: () => void;
  isTesting: boolean;
}) {
  const statusParts = [
    !webhook.active ? t`Paused` : null,
    !webhook.active ? t`not receiving events` : null,
    webhook.events.length > 0 ? webhook.events.join(", ") : t`All events`,
    webhook.last_delivery_at
      ? t`Last delivery ${webhook.last_delivery_status}`
      : null,
  ].filter(Boolean);

  return (
    <li {...stylex.props(styles.row)}>
      <div {...stylex.props(styles.identity)}>
        <span
          {...stylex.props(styles.url, !webhook.active && styles.inactiveUrl)}
        >
          {webhook.url}
        </span>
        <span {...stylex.props(styles.meta)}>{statusParts.join(" · ")}</span>
      </div>
      <div {...stylex.props(styles.actions)}>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          sx={styles.rowButton}
          onClick={onToggleActive}
        >
          {webhook.active ? t`Pause` : t`Enable`}
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          sx={styles.rowButton}
          disabled={isTesting}
          onClick={onTest}
        >
          {t`Test`}
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          sx={[styles.rowButton, styles.deleteButton]}
          onClick={onDelete}
        >
          <Trash {...stylex.props(styles.icon)} />
        </Button>
      </div>
    </li>
  );
}

const styles = stylex.create({
  actions: {
    alignItems: "center",
    display: "flex",
    flexShrink: 0,
    gap: "0.25rem",
  },
  code: {
    backgroundColor: colors.muted,
    borderRadius: radii.md,
    display: {
      default: null,
      "::-webkit-scrollbar": "none",
    },
    fontSize: "0.75rem",
    lineHeight: "1rem",
    overflowX: "auto",
    paddingBlock: "0.125rem",
    paddingInline: "0.375rem",
    scrollbarWidth: "none",
  },
  copyButton: {
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
  deleteButton: {
    color: colors.destructive,
  },
  form: {
    display: "flex",
    gap: "0.5rem",
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
    display: "flex",
    flexDirection: "column",
    minWidth: 0,
  },
  inactiveUrl: {
    color: colors.mutedForeground,
    textDecorationLine: "line-through",
  },
  input: {
    fontSize: "0.875rem",
    height: "2rem",
    lineHeight: "1.25rem",
    maxWidth: "28rem",
  },
  list: {
    display: "flex",
    flexDirection: "column",
    gap: "0.375rem",
    marginTop: "0.75rem",
  },
  meta: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  mutedText: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  row: {
    alignItems: "center",
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.75rem",
    justifyContent: "space-between",
    lineHeight: "1.25rem",
  },
  rowButton: {
    height: "1.75rem",
  },
  secretRow: {
    alignItems: "center",
    display: "flex",
    gap: "0.5rem",
    marginTop: "0.5rem",
  },
  section: {
    display: "flex",
    flexDirection: "column",
    gap: "1rem",
  },
  url: {
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
});
