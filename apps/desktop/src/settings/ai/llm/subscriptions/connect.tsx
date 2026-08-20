import { Trans, useLingui } from "@lingui/react/macro";
import { CircleNotch, Copy, Plus } from "@phosphor-icons/react";
import { useMutation } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";

import { commands as analyticsCommands } from "@anlg/plugin-analytics";
import { commands as openerCommands } from "@anlg/plugin-opener2";
import { Button } from "@anlg/ui/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@anlg/ui/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@anlg/ui/components/ui/dropdown-menu";
import { Input } from "@anlg/ui/components/ui/input";
import { cn } from "@anlg/utils";

import { type Provider } from "../shared";
import {
  completeCodeConnect,
  type ConnectSession,
  isSubscriptionProviderId,
  pollDeviceConnect,
  startSubscriptionConnect,
  type SubscriptionProviderId,
} from "./oauth";

import { ProviderIconSlot } from "~/settings/ai/shared";
import { useProviderSelectionPrompt } from "~/settings/ai/shared/provider-selection-prompt";
import { useSetAiProvider } from "~/settings/providers";
import { useConfigValue } from "~/shared/config";

export function ConnectProvidersMenu({
  providers,
  configuredIds,
  connectingId,
  onConnectingIdChange,
  onSelectApiProvider,
}: {
  providers: readonly Provider[];
  configuredIds: ReadonlySet<string>;
  connectingId: SubscriptionProviderId | null;
  onConnectingIdChange: (providerId: SubscriptionProviderId | null) => void;
  onSelectApiProvider: (providerId: string) => void;
}) {
  const { t } = useLingui();
  const subscriptionProviders = providers.filter(
    (provider) =>
      provider.authKind === "subscription" && !configuredIds.has(provider.id),
  );
  const apiProviders = providers.filter(
    (provider) =>
      provider.id !== "anarlog" &&
      provider.authKind !== "subscription" &&
      !configuredIds.has(provider.id),
  );

  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button
            type="button"
            className={cn([
              "bg-muted text-muted-foreground hover:text-foreground flex w-full items-center justify-center gap-2 rounded-[22px] border-2 border-dashed px-4 py-3 text-sm font-medium transition-colors",
            ])}
          >
            <Plus className="size-4" />
            <Trans>Connect</Trans>
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent variant="app" align="start" className="w-64">
          {subscriptionProviders.length > 0 ? (
            <>
              <DropdownMenuLabel className="text-muted-foreground text-xs font-medium">
                <Trans>Subscription</Trans>
              </DropdownMenuLabel>
              {subscriptionProviders.map((provider) => (
                <DropdownMenuItem
                  key={provider.id}
                  onSelect={() => {
                    if (isSubscriptionProviderId(provider.id)) {
                      onConnectingIdChange(provider.id);
                    }
                  }}
                >
                  <ProviderIconSlot>{provider.icon}</ProviderIconSlot>
                  <span>{provider.displayName}</span>
                </DropdownMenuItem>
              ))}
            </>
          ) : null}
          {subscriptionProviders.length > 0 && apiProviders.length > 0 ? (
            <DropdownMenuSeparator />
          ) : null}
          {apiProviders.length > 0 ? (
            <>
              <DropdownMenuLabel className="text-muted-foreground text-xs font-medium">
                <Trans>API</Trans>
              </DropdownMenuLabel>
              {apiProviders.map((provider) => (
                <DropdownMenuItem
                  key={provider.id}
                  onSelect={() => onSelectApiProvider(provider.id)}
                >
                  <ProviderIconSlot>{provider.icon}</ProviderIconSlot>
                  <span>{provider.displayName}</span>
                </DropdownMenuItem>
              ))}
            </>
          ) : null}
          {subscriptionProviders.length === 0 && apiProviders.length === 0 ? (
            <div className="text-muted-foreground px-2 py-1.5 text-sm">
              {t`All providers are connected.`}
            </div>
          ) : null}
        </DropdownMenuContent>
      </DropdownMenu>
      <ConnectSubscriptionDialog
        provider={
          connectingId
            ? providers.find((provider) => provider.id === connectingId)
            : undefined
        }
        onOpenChange={(open) => {
          if (!open) {
            onConnectingIdChange(null);
          }
        }}
      />
    </>
  );
}

export function ConnectSubscriptionDialog({
  provider,
  onOpenChange,
}: {
  provider?: Provider;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useLingui();
  const currentProvider = useConfigValue("current_llm_provider");
  const providerId =
    provider && isSubscriptionProviderId(provider.id) ? provider.id : null;
  const saveProvider = useSetAiProvider("llm", providerId ?? "claude");
  const notifyProviderSelection = useProviderSelectionPrompt({
    providerType: "llm",
    providerId: providerId ?? "claude",
    providerName: provider?.displayName ?? "Claude",
    currentProvider,
    providerStateReady: true,
    storedApiKey: "",
  });
  const [session, setSession] = useState<ConnectSession | null>(null);
  const [code, setCode] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [isStarting, setIsStarting] = useState(false);
  const pollRef = useRef<number | null>(null);
  const saveProviderRef = useRef(saveProvider);
  const notifyProviderSelectionRef = useRef(notifyProviderSelection);
  const onOpenChangeRef = useRef(onOpenChange);
  saveProviderRef.current = saveProvider;
  notifyProviderSelectionRef.current = notifyProviderSelection;
  onOpenChangeRef.current = onOpenChange;

  const completeMutation = useMutation({
    mutationFn: async () => {
      if (!providerId || !provider) {
        throw new Error("No provider selected.");
      }
      if (providerId === "kimi_code") {
        const key = apiKey.trim();
        if (!key) {
          throw new Error("Paste your Kimi Code API key.");
        }
        await saveProvider.mutateAsync({
          base_url: provider.baseUrl,
          api_key: key,
        });
        return key;
      }
      if (!session || session.kind === "api_key") {
        throw new Error("Sign-in is not ready yet.");
      }
      if (session.kind === "code") {
        if (providerId !== "claude" && providerId !== "chatgpt") {
          throw new Error("This provider uses a different sign-in flow.");
        }
        const stored = await completeCodeConnect(providerId, session, code);
        await saveProvider.mutateAsync({
          base_url: provider.baseUrl,
          api_key: stored,
        });
        return stored;
      }
      throw new Error("Waiting for authorization in the browser.");
    },
    onSuccess: (stored) => {
      notifyProviderSelection(stored);
      void analyticsCommands.event({
        event: "ai_provider_configured",
        provider: "llm",
      });
      onOpenChange(false);
    },
    onError: (caught) => {
      setError(caught instanceof Error ? caught.message : String(caught));
    },
  });

  useEffect(() => {
    if (!providerId) {
      setSession(null);
      setCode("");
      setApiKey("");
      setError(null);
      setIsStarting(false);
      return;
    }

    let cancelled = false;

    setSession(null);
    setCode("");
    setApiKey("");
    setError(null);
    setIsStarting(true);
    void (async () => {
      try {
        const next = await startSubscriptionConnect(providerId);
        if (next.kind === "code" || next.kind === "device") {
          const opened = await openerCommands.openUrl(
            next.kind === "code" ? next.url : next.verificationUrl,
            null,
          );
          if (opened.status === "error") {
            throw new Error(opened.error);
          }
        }
        if (!cancelled) {
          setSession(next);
        }
      } catch (caught) {
        if (!cancelled) {
          setError(caught instanceof Error ? caught.message : String(caught));
        }
      } finally {
        if (!cancelled) {
          setIsStarting(false);
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [providerId]);

  useEffect(() => {
    if (
      !providerId ||
      !provider ||
      !session ||
      session.kind !== "device" ||
      (providerId !== "github_copilot" && providerId !== "grok")
    ) {
      return;
    }

    let cancelled = false;
    const poll = async () => {
      try {
        const result = await pollDeviceConnect(providerId, session);
        if (cancelled) {
          return;
        }
        if (result === "pending") {
          pollRef.current = window.setTimeout(poll, session.intervalMs);
          return;
        }
        await saveProviderRef.current.mutateAsync({
          base_url: provider.baseUrl,
          api_key: result,
        });
        notifyProviderSelectionRef.current(result);
        void analyticsCommands.event({
          event: "ai_provider_configured",
          provider: "llm",
        });
        onOpenChangeRef.current(false);
      } catch (caught) {
        if (!cancelled) {
          setError(caught instanceof Error ? caught.message : String(caught));
        }
      }
    };
    pollRef.current = window.setTimeout(poll, session.intervalMs);
    return () => {
      cancelled = true;
      if (pollRef.current !== null) {
        window.clearTimeout(pollRef.current);
      }
    };
  }, [provider, providerId, session]);

  const copyUserCode = async () => {
    if (session?.kind !== "device") {
      return;
    }
    await navigator.clipboard.writeText(session.userCode);
  };

  return (
    <Dialog open={!!provider} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>
            {provider ? t`Connect ${provider.displayName}` : t`Connect`}
          </DialogTitle>
          <DialogDescription>
            {providerId === "claude"
              ? t`Sign in with Claude Pro or Max, then paste the authorization code.`
              : providerId === "chatgpt"
                ? t`Sign in with ChatGPT Plus or Pro, then paste the redirect URL from your browser.`
                : providerId === "github_copilot"
                  ? t`Sign in with GitHub Copilot and enter the code below.`
                  : providerId === "grok"
                    ? t`Sign in with SuperGrok or X Premium+ and enter the code below.`
                    : t`Paste an API key from your Kimi Code membership.`}
          </DialogDescription>
        </DialogHeader>
        {isStarting ? (
          <div className="text-muted-foreground flex items-center gap-2 text-sm">
            <CircleNotch className="size-4 animate-spin" />
            <Trans>Opening sign-in…</Trans>
          </div>
        ) : null}
        {session?.kind === "code" ? (
          <div className="flex flex-col gap-2">
            <Input
              value={code}
              onChange={(event) => setCode(event.target.value)}
              placeholder={
                providerId === "chatgpt"
                  ? t`Paste the localhost redirect URL`
                  : t`Paste the authorization code`
              }
              autoFocus
            />
          </div>
        ) : null}
        {session?.kind === "device" ? (
          <div className="flex flex-col gap-3">
            <div className="bg-muted flex items-center justify-between rounded-xl px-3 py-2">
              <span className="font-mono text-lg tracking-widest">
                {session.userCode}
              </span>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={() => void copyUserCode()}
              >
                <Copy className="size-4" />
                <Trans>Copy</Trans>
              </Button>
            </div>
            <p className="text-muted-foreground text-xs">
              <Trans>Waiting for authorization in your browser…</Trans>
            </p>
          </div>
        ) : null}
        {session?.kind === "api_key" ? (
          <div className="flex flex-col gap-2">
            <Input
              type="password"
              value={apiKey}
              onChange={(event) => setApiKey(event.target.value)}
              placeholder={t`Kimi Code API key`}
              autoFocus
            />
            <a
              href={session.docsUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="text-muted-foreground hover:text-foreground text-xs underline"
            >
              <Trans>How to get a Kimi Code key</Trans>
            </a>
          </div>
        ) : null}
        {error ? <p className="text-destructive text-xs">{error}</p> : null}
        <DialogFooter>
          <Button
            type="button"
            variant="ghost"
            onClick={() => onOpenChange(false)}
          >
            <Trans>Cancel</Trans>
          </Button>
          {session?.kind === "code" || session?.kind === "api_key" ? (
            <Button
              type="button"
              onClick={() => completeMutation.mutate()}
              disabled={completeMutation.isPending}
            >
              {completeMutation.isPending ? (
                <CircleNotch className="size-4 animate-spin" />
              ) : null}
              <Trans>Connect</Trans>
            </Button>
          ) : null}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
