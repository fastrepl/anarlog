import { Trans, useLingui } from "@lingui/react/macro";
import { CircleNotch, Copy } from "@phosphor-icons/react";
import * as stylex from "@stylexjs/stylex";
import { useMutation } from "@tanstack/react-query";
import { readText as readClipboardText } from "@tauri-apps/plugin-clipboard-manager";
import { useEffect, useRef, useState } from "react";

import { colors, fonts, radii } from "@anlg/design-system/tokens.stylex";
import { commands as analyticsCommands } from "@anlg/plugin-analytics";
import {
  commands as deeplink2Commands,
  events as deeplink2Events,
} from "@anlg/plugin-deeplink2";
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
import { Input } from "@anlg/ui/components/ui/input";

import { type Provider } from "../shared";
import {
  authorizationInputFromParsed,
  CHATGPT_CALLBACK_PORT,
  completeCodeConnect,
  type ConnectSession,
  isSubscriptionProviderId,
  looksLikeAuthorizationInput,
  pollDeviceConnect,
  startSubscriptionConnect,
  subscriptionAuthFromCallback,
} from "./oauth";

import { useProviderSelectionPrompt } from "~/settings/ai/shared/provider-selection-prompt";
import { useSetAiProvider } from "~/settings/providers";
import { useConfigValue } from "~/shared/config";
import { getScheme } from "~/shared/utils";

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
  const [listeningForCallback, setListeningForCallback] = useState(false);
  const [showPasteFallback, setShowPasteFallback] = useState(false);
  const pollRef = useRef<number | null>(null);
  const saveProviderRef = useRef(saveProvider);
  const notifyProviderSelectionRef = useRef(notifyProviderSelection);
  const onOpenChangeRef = useRef(onOpenChange);
  const sessionRef = useRef(session);
  const pendingCodeRef = useRef<string | null>(null);
  const completingRef = useRef(false);
  saveProviderRef.current = saveProvider;
  notifyProviderSelectionRef.current = notifyProviderSelection;
  onOpenChangeRef.current = onOpenChange;
  sessionRef.current = session;

  const finishConnect = async (stored: string) => {
    notifyProviderSelection(stored);
    void analyticsCommands.event({
      event: "ai_provider_configured",
      provider: "llm",
    });
    onOpenChange(false);
  };

  const completeMutation = useMutation({
    mutationFn: async (rawCode?: string) => {
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
        const stored = await completeCodeConnect(
          providerId,
          session,
          rawCode ?? code,
        );
        await saveProvider.mutateAsync({
          base_url: provider.baseUrl,
          api_key: stored,
        });
        return stored;
      }
      throw new Error("Waiting for authorization in the browser.");
    },
    onSuccess: (stored) => {
      void finishConnect(stored);
    },
    onError: (caught) => {
      completingRef.current = false;
      setError(caught instanceof Error ? caught.message : String(caught));
    },
  });

  const completeFromAuthorization = (rawCode: string) => {
    if (completingRef.current) {
      return;
    }
    completingRef.current = true;
    setCode(rawCode);
    setError(null);
    completeMutation.mutate(rawCode);
  };
  const completeFromAuthorizationRef = useRef(completeFromAuthorization);
  completeFromAuthorizationRef.current = completeFromAuthorization;

  useEffect(() => {
    if (!providerId) {
      setSession(null);
      setCode("");
      setApiKey("");
      setError(null);
      setIsStarting(false);
      setListeningForCallback(false);
      setShowPasteFallback(false);
      completingRef.current = false;
      return;
    }

    let cancelled = false;
    const loopback = { started: false };

    setSession(null);
    setCode("");
    setApiKey("");
    setError(null);
    setIsStarting(true);
    setListeningForCallback(false);
    setShowPasteFallback(false);
    pendingCodeRef.current = null;
    completingRef.current = false;
    void (async () => {
      try {
        if (providerId === "chatgpt") {
          try {
            const scheme = await getScheme();
            const started = await deeplink2Commands.startCallbackServer(
              scheme,
              CHATGPT_CALLBACK_PORT,
            );
            if (
              started.status === "ok" &&
              started.data === CHATGPT_CALLBACK_PORT
            ) {
              if (cancelled) {
                await deeplink2Commands.stopCallbackServer();
                return;
              }
              loopback.started = true;
              setListeningForCallback(true);
            } else if (!cancelled) {
              setShowPasteFallback(true);
            }
          } catch {
            if (!cancelled) {
              setShowPasteFallback(true);
            }
          }
        }

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
      if (loopback.started) {
        void deeplink2Commands.stopCallbackServer();
      }
    };
  }, [providerId]);

  useEffect(() => {
    if (providerId !== "chatgpt" && providerId !== "claude") {
      return;
    }

    let cancelled = false;
    const subscription = deeplink2Events.deepLinkEvent.listen(({ payload }) => {
      if (cancelled || payload.to !== "/auth/callback") {
        return;
      }
      const parsed = subscriptionAuthFromCallback(payload.search);
      if (!parsed) {
        return;
      }
      const raw = authorizationInputFromParsed(parsed);
      const current = sessionRef.current;
      if (!current || current.kind !== "code") {
        pendingCodeRef.current = raw;
        return;
      }
      if (parsed.state && parsed.state !== current.state) {
        setError("This sign-in expired. Try connecting again.");
        return;
      }
      completeFromAuthorizationRef.current(raw);
    });

    return () => {
      cancelled = true;
      void subscription.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [providerId]);

  useEffect(() => {
    if (session?.kind !== "code" || !pendingCodeRef.current) {
      return;
    }
    const pending = pendingCodeRef.current;
    pendingCodeRef.current = null;
    completeFromAuthorizationRef.current(pending);
  }, [session]);

  useEffect(() => {
    if (providerId !== "claude" || session?.kind !== "code") {
      return;
    }

    let cancelled = false;
    const pollClipboard = async () => {
      try {
        const text = await readClipboardText();
        if (cancelled || !looksLikeAuthorizationInput(text)) {
          return;
        }
        completeFromAuthorizationRef.current(text);
      } catch {
        // Clipboard permission or empty clipboard — keep the paste field.
      }
    };

    const intervalId = window.setInterval(() => {
      void pollClipboard();
    }, 1000);
    void pollClipboard();

    return () => {
      cancelled = true;
      window.clearInterval(intervalId);
    };
  }, [providerId, session]);

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

  const showCodeInput =
    session?.kind === "code" &&
    (providerId !== "chatgpt" || showPasteFallback || !listeningForCallback);

  return (
    <Dialog open={!!provider} onOpenChange={onOpenChange}>
      <DialogContent sx={styles.dialog}>
        <DialogHeader>
          <DialogTitle>
            {provider ? t`Connect ${provider.displayName}` : t`Connect`}
          </DialogTitle>
          <DialogDescription>
            {providerId === "claude"
              ? t`Sign in with Claude Pro or Max. After you authorize, we'll pick up the code and finish connecting.`
              : providerId === "chatgpt"
                ? listeningForCallback
                  ? t`Sign in with ChatGPT Plus or Pro. We'll open Anarlog and finish connecting.`
                  : t`Sign in with ChatGPT Plus or Pro, then paste the redirect URL from your browser.`
                : providerId === "github_copilot"
                  ? t`Sign in with GitHub Copilot and enter the code below.`
                  : providerId === "grok"
                    ? t`Sign in with SuperGrok or X Premium+ and enter the code below.`
                    : t`Paste an API key from your Kimi Code membership.`}
          </DialogDescription>
        </DialogHeader>
        {isStarting ||
        completeMutation.isPending ||
        (session?.kind === "code" &&
          (listeningForCallback || providerId === "claude")) ? (
          <div {...stylex.props(styles.loading)}>
            <CircleNotch {...stylex.props(styles.spinner)} />
            {isStarting ? (
              <Trans>Opening sign-in…</Trans>
            ) : completeMutation.isPending ? (
              <Trans>Finishing connection…</Trans>
            ) : (
              <Trans>Waiting for authorization in your browser…</Trans>
            )}
          </div>
        ) : null}
        {session?.kind === "code" &&
        listeningForCallback &&
        !showPasteFallback ? (
          <button
            type="button"
            {...stylex.props(styles.troubleButton)}
            onClick={() => setShowPasteFallback(true)}
          >
            <Trans>Having trouble? Paste the redirect URL</Trans>
          </button>
        ) : null}
        {showCodeInput ? (
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
        ) : null}
        {session?.kind === "device" ? (
          <div {...stylex.props(styles.device)}>
            <div {...stylex.props(styles.deviceCode)}>
              <span {...stylex.props(styles.code)}>{session.userCode}</span>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={() => void copyUserCode()}
              >
                <Copy {...stylex.props(styles.icon)} />
                <Trans>Copy</Trans>
              </Button>
            </div>
            <p {...stylex.props(styles.message)}>
              <Trans>Waiting for authorization in your browser…</Trans>
            </p>
          </div>
        ) : null}
        {session?.kind === "api_key" ? (
          <div {...stylex.props(styles.apiKey)}>
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
              {...stylex.props(styles.docsLink)}
            >
              <Trans>How to get a Kimi Code key</Trans>
            </a>
          </div>
        ) : null}
        {error ? <p {...stylex.props(styles.error)}>{error}</p> : null}
        <DialogFooter>
          <Button
            type="button"
            variant="ghost"
            onClick={() => onOpenChange(false)}
          >
            <Trans>Cancel</Trans>
          </Button>
          {showCodeInput || session?.kind === "api_key" ? (
            <Button
              type="button"
              onClick={() => completeMutation.mutate(undefined)}
              disabled={completeMutation.isPending}
            >
              {completeMutation.isPending ? (
                <CircleNotch {...stylex.props(styles.spinner)} />
              ) : null}
              <Trans>Connect</Trans>
            </Button>
          ) : null}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

const spin = stylex.keyframes({
  to: { transform: "rotate(360deg)" },
});

const styles = stylex.create({
  apiKey: {
    display: "flex",
    flexDirection: "column",
    gap: "0.5rem",
  },
  code: {
    fontFamily: fonts.mono,
    fontSize: "1.125rem",
    letterSpacing: "0.1em",
    lineHeight: "1.75rem",
  },
  device: {
    display: "flex",
    flexDirection: "column",
    gap: "0.75rem",
  },
  deviceCode: {
    alignItems: "center",
    backgroundColor: colors.muted,
    borderRadius: radii.xl,
    display: "flex",
    justifyContent: "space-between",
    paddingBlock: "0.5rem",
    paddingInline: "0.75rem",
  },
  dialog: {
    maxWidth: "28rem",
  },
  docsLink: {
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    fontSize: "0.75rem",
    lineHeight: "1rem",
    textDecorationLine: "underline",
  },
  error: {
    color: colors.destructive,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  icon: {
    height: "1rem",
    width: "1rem",
  },
  loading: {
    alignItems: "center",
    color: colors.mutedForeground,
    display: "flex",
    fontSize: "0.875rem",
    gap: "0.5rem",
    lineHeight: "1.25rem",
  },
  message: {
    color: colors.mutedForeground,
    fontSize: "0.75rem",
    lineHeight: "1rem",
  },
  spinner: {
    animationDuration: "1s",
    animationIterationCount: "infinite",
    animationName: spin,
    animationTimingFunction: "linear",
    height: "1rem",
    width: "1rem",
  },
  troubleButton: {
    alignSelf: "flex-start",
    color: {
      default: colors.mutedForeground,
      ":hover": colors.foreground,
    },
    fontSize: "0.75rem",
    lineHeight: "1rem",
    textDecorationLine: "underline",
  },
});
