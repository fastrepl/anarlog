import {
  ArrowSquareOut,
  CheckCircle,
  Copy,
  Globe,
  Key,
  Trash,
  WebhooksLogo,
} from "@phosphor-icons/react";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  commands as localApiCommands,
  type WebhookInfo,
} from "@anlg/plugin-local-api";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { Button } from "@anlg/ui/components/ui/button";
import { Input } from "@anlg/ui/components/ui/input";
import { Switch } from "@anlg/ui/components/ui/switch";
import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { ApiKeyRow } from "./api-key-row";
import { copyText } from "./clipboard";

const API_GUIDE_URL = "https://docs.anarlog.so/reference/api";
const API_STATUS_QUERY_KEY = ["local-api", "status"] as const;
const API_KEYS_QUERY_KEY = ["local-api", "keys"] as const;
const API_WEBHOOKS_QUERY_KEY = ["local-api", "webhooks"] as const;

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

export function LocalApiSection() {
  const queryClient = useQueryClient();

  const statusQuery = useQuery({
    queryKey: API_STATUS_QUERY_KEY,
    queryFn: () => unwrap(localApiCommands.getStatus()),
  });
  const toggleMutation = useMutation({
    mutationFn: (enabled: boolean) =>
      unwrap(localApiCommands.setEnabled(enabled)),
    onSuccess: (status) => {
      queryClient.setQueryData(API_STATUS_QUERY_KEY, status);
    },
    onError: (error) => sonnerToast.error(error.message),
  });

  const status = statusQuery.data;
  const running = status?.running === true;

  return (
    <section className="flex flex-col gap-3">
      <h2 className="text-muted-foreground text-sm font-medium">Local API</h2>
      <div className="border-border bg-card overflow-hidden rounded-2xl border">
        <div className="flex flex-col gap-4 p-4 sm:flex-row sm:items-start sm:justify-between">
          <div className="flex min-w-0 gap-3">
            <div className="bg-muted flex size-10 shrink-0 items-center justify-center rounded-xl">
              <Globe className="size-5" />
            </div>
            <div className="min-w-0">
              <h3 className="font-medium">Anarlog local API</h3>
              <p className="text-muted-foreground mt-1 text-sm leading-5">
                Connect local tools to your meetings without sending data
                off-device.
              </p>
              <p className="text-muted-foreground mt-2 flex items-center gap-1.5 text-xs">
                {running ? (
                  <>
                    <CheckCircle className="size-3.5 shrink-0 text-emerald-600" />
                    <code className="bg-muted rounded-md px-1.5 py-0.5">
                      http://127.0.0.1:{status?.port}
                    </code>
                  </>
                ) : (
                  <>
                    <span className="bg-muted-foreground/50 size-2 shrink-0 rounded-full" />
                    Server is off
                  </>
                )}
              </p>
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => void openerCommands.openUrl(API_GUIDE_URL, null)}
            >
              Guide
              <ArrowSquareOut className="size-3.5" />
            </Button>
            <Switch
              checked={status?.enabled === true}
              disabled={statusQuery.isPending || toggleMutation.isPending}
              onCheckedChange={(checked) => toggleMutation.mutate(checked)}
            />
          </div>
        </div>

        <ApiKeysCard />
        <WebhooksCard />
      </div>
    </section>
  );
}

function ApiKeysCard() {
  const queryClient = useQueryClient();
  const keysQuery = useQuery({
    queryKey: API_KEYS_QUERY_KEY,
    queryFn: () => unwrap(localApiCommands.listApiKeys()),
  });
  const createMutation = useMutation({
    mutationFn: (name: string) => unwrap(localApiCommands.createApiKey(name)),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: API_KEYS_QUERY_KEY }),
    onError: (error) => sonnerToast.error(error.message),
  });
  const revokeMutation = useMutation({
    mutationFn: (id: string) => unwrap(localApiCommands.revokeApiKey(id)),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: API_KEYS_QUERY_KEY }),
    onError: (error) => sonnerToast.error(error.message),
  });

  const form = useForm({
    defaultValues: { name: "" },
    onSubmit: ({ value }) => {
      createMutation.mutate(value.name.trim() || "API key");
      form.setFieldValue("name", "");
    },
  });

  const createdKey = createMutation.data;

  return (
    <div className="border-border border-t p-4">
      <div className="mb-3 flex items-center gap-2">
        <Key className="text-muted-foreground size-4" />
        <h4 className="text-sm font-medium">API keys</h4>
      </div>

      <form
        className="flex gap-2"
        onSubmit={(event) => {
          event.preventDefault();
          event.stopPropagation();
          void form.handleSubmit();
        }}
      >
        <form.Field name="name">
          {(field) => (
            <Input
              className="h-8 max-w-64 text-sm"
              placeholder="Key name (e.g. Zapier)"
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
          Create key
        </Button>
      </form>

      {createdKey && (
        <div className="border-border bg-muted/30 mt-3 rounded-xl border p-3">
          <p className="text-muted-foreground text-xs">
            Copy this key now — it is only shown once.
          </p>
          <div className="mt-2 flex items-center gap-2">
            <code className="bg-muted scrollbar-hide overflow-x-auto rounded-md px-1.5 py-0.5 text-xs">
              {createdKey.key}
            </code>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-7 shrink-0"
              onClick={async () => {
                if (await copyText(createdKey.key, "API key copied")) {
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
        {(keysQuery.data ?? []).map((key) => (
          <ApiKeyRow
            key={key.id}
            apiKey={key}
            onRevoke={() => revokeMutation.mutate(key.id)}
          />
        ))}
        {keysQuery.data?.length === 0 && !createdKey && (
          <li className="text-muted-foreground text-xs">
            No API keys yet. Create one to call the API.
          </li>
        )}
      </ul>
    </div>
  );
}

function WebhooksCard() {
  const queryClient = useQueryClient();
  const webhooksQuery = useQuery({
    queryKey: API_WEBHOOKS_QUERY_KEY,
    queryFn: () => unwrap(localApiCommands.listWebhooks()),
  });
  const createMutation = useMutation({
    mutationFn: (url: string) =>
      unwrap(localApiCommands.createWebhook(url, [])),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: API_WEBHOOKS_QUERY_KEY }),
    onError: (error) => sonnerToast.error(error.message),
  });
  const deleteMutation = useMutation({
    mutationFn: (id: string) => unwrap(localApiCommands.deleteWebhook(id)),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: API_WEBHOOKS_QUERY_KEY }),
    onError: (error) => sonnerToast.error(error.message),
  });
  const testMutation = useMutation({
    mutationFn: (id: string) => unwrap(localApiCommands.testWebhook(id)),
    onSuccess: (delivery) => {
      if (delivery.delivered) {
        sonnerToast.success(`Test delivered (${delivery.status})`);
      } else {
        sonnerToast.error(`Test failed (${delivery.status})`);
      }
      void queryClient.invalidateQueries({ queryKey: API_WEBHOOKS_QUERY_KEY });
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
    <div className="border-border border-t p-4">
      <div className="mb-3 flex items-center gap-2">
        <WebhooksLogo className="text-muted-foreground size-4" />
        <h4 className="text-sm font-medium">Webhooks</h4>
      </div>
      <p className="text-muted-foreground mb-3 text-xs">
        Receive signed events when meetings end or summaries are ready.
      </p>

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
                  await copyText(createdWebhook.secret, "Signing secret copied")
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
  );
}

function WebhookRow({
  webhook,
  onTest,
  onDelete,
  isTesting,
}: {
  webhook: WebhookInfo;
  onTest: () => void;
  onDelete: () => void;
  isTesting: boolean;
}) {
  return (
    <li className="flex items-center justify-between gap-3 text-sm">
      <div className="flex min-w-0 flex-col">
        <span className="truncate">{webhook.url}</span>
        <span className="text-muted-foreground text-xs">
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
