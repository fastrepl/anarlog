import {
  ArrowSquareOut,
  Copy,
  Trash,
  WebhooksLogo,
} from "@phosphor-icons/react";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  commands as webhookCommands,
  type WebhookInfo,
} from "@anlg/plugin-local-api";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { Button } from "@anlg/ui/components/ui/button";
import { Input } from "@anlg/ui/components/ui/input";
import { sonnerToast } from "@anlg/ui/components/ui/toast";
import { cn } from "@anlg/utils";

import { copyText } from "./clipboard";

const WEBHOOKS_GUIDE_URL = "https://docs.anarlog.so/reference/webhooks";
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
        sonnerToast.success(`Test delivered (${delivery.status})`);
      } else {
        sonnerToast.error(`Test failed (${delivery.status})`);
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

  return (
    <section className="flex flex-col gap-3">
      <h2 className="text-muted-foreground text-sm font-medium">Webhooks</h2>
      <div className="border-border bg-card overflow-hidden rounded-2xl border">
        <div className="flex flex-col gap-4 p-4 sm:flex-row sm:items-start sm:justify-between">
          <div className="flex min-w-0 gap-3">
            <div className="bg-muted flex size-10 shrink-0 items-center justify-center rounded-xl">
              <WebhooksLogo className="size-5" />
            </div>
            <div className="min-w-0">
              <h3 className="font-medium">Meeting events</h3>
              <p className="text-muted-foreground mt-1 text-sm leading-5">
                Receive signed events when meetings end or summaries are ready.
                Deliveries include meeting content, so only add an endpoint you
                trust.
              </p>
            </div>
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="shrink-0"
            onClick={() =>
              void openerCommands.openUrl(WEBHOOKS_GUIDE_URL, null)
            }
          >
            Guide
            <ArrowSquareOut className="size-3.5" />
          </Button>
        </div>

        <div className="border-border border-t p-4">
          <form
            className="flex gap-2"
            onSubmit={(event) => {
              event.preventDefault();
              event.stopPropagation();
              void form.handleSubmit();
            }}
          >
            <form.Field name="url">
              {(field) => (
                <Input
                  className="h-8 max-w-md text-sm"
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
              Add webhook
            </Button>
          </form>

          {createdWebhook && (
            <div className="border-border bg-muted/30 mt-3 rounded-xl border p-3">
              <p className="text-muted-foreground text-xs">
                Signing secret — copy it now, it is only shown once. Use it to
                verify the <code>x-anarlog-signature</code> header.
              </p>
              <div className="mt-2 flex items-center gap-2">
                <code className="bg-muted scrollbar-hide overflow-x-auto rounded-md px-1.5 py-0.5 text-xs">
                  {createdWebhook.secret}
                </code>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="h-7 shrink-0"
                  onClick={async () => {
                    if (
                      await copyText(
                        createdWebhook.secret,
                        "Signing secret copied",
                      )
                    ) {
                      createMutation.reset();
                    }
                  }}
                >
                  <Copy className="size-3.5" />
                  Copy
                </Button>
              </div>
            </div>
          )}

          <ul className="mt-3 flex flex-col gap-1.5">
            {(webhooksQuery.data ?? []).map((webhook) => (
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
            {webhooksQuery.data?.length === 0 && !createdWebhook && (
              <li className="text-muted-foreground text-xs">
                No webhooks yet. Add an endpoint to receive events.
              </li>
            )}
          </ul>
        </div>
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
  return (
    <li className="flex items-center justify-between gap-3 text-sm">
      <div className="flex min-w-0 flex-col">
        <span
          className={cn([
            "truncate",
            !webhook.active && "text-muted-foreground line-through",
          ])}
        >
          {webhook.url}
        </span>
        <span className="text-muted-foreground text-xs">
          {!webhook.active && "Paused · not receiving events · "}
          {webhook.events.length > 0 ? webhook.events.join(", ") : "All events"}
          {webhook.last_delivery_at &&
            ` · Last delivery ${webhook.last_delivery_status}`}
        </span>
      </div>
      <div className="flex shrink-0 items-center gap-1">
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="h-7"
          onClick={onToggleActive}
        >
          {webhook.active ? "Pause" : "Enable"}
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="h-7"
          disabled={isTesting}
          onClick={onTest}
        >
          Test
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="text-destructive h-7"
          onClick={onDelete}
        >
          <Trash className="size-3.5" />
        </Button>
      </div>
    </li>
  );
}
