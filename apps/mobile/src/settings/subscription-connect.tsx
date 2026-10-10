import { Column, Row, useNativeState } from "@expo/ui";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CryptoDigestAlgorithm,
  CryptoEncoding,
  digestStringAsync,
  getRandomBytes,
} from "expo-crypto";
import { useFocusEffect } from "expo-router";
import { fetch } from "expo/fetch";
import { useCallback, useRef } from "react";
import { Linking } from "react-native";

import { Button, Text, TextInput } from "./fields";
import {
  readProviderSetup,
  removeProviderKey,
  saveProviderSetup,
} from "./providers";
import { saveSubscriptionCredential } from "./subscription-access";
import {
  completeClaudeConnect,
  listSubscriptionModels,
  pollSubscriptionConnect,
  startSubscriptionConnect,
  type OAuthSubscriptionProvider,
  type SubscriptionCredential,
} from "./subscription-oauth";
import { useColors } from "./theme-provider";

export function SubscriptionConnect({
  provider,
  account,
  connected,
  verificationError,
  onSaved,
}: {
  provider: OAuthSubscriptionProvider;
  account: string | null;
  connected: boolean;
  verificationError?: string;
  onSaved: () => void;
}) {
  const Colors = useColors();
  const name =
    provider === "chatgpt"
      ? "ChatGPT"
      : provider === "claude"
        ? "Claude"
        : provider === "grok"
          ? "Grok"
          : "GitHub Copilot";
  const code = useNativeState("");
  const form = useForm({ defaultValues: { code: "" } });
  const queryClient = useQueryClient();
  const controller = useRef<AbortController | null>(null);
  const starting = useRef(false);
  const completing = useRef(false);
  const cancel = useCallback(() => {
    controller.current?.abort();
    void queryClient.cancelQueries({
      queryKey: ["subscription-connect", account, provider],
    });
  }, [account, provider, queryClient]);
  const invalidate = async () => {
    await queryClient.invalidateQueries({
      queryKey: ["provider-setup", account, "llm", provider],
    });
    await queryClient.invalidateQueries({
      queryKey: ["provider", account, "llm"],
    });
    await queryClient.resetQueries({
      queryKey: ["provider-models", account, "llm", provider],
    });
  };
  const start = useMutation({
    gcTime: 0,
    mutationFn: async () => {
      cancel();
      const current = new AbortController();
      controller.current = current;
      form.reset();
      code.value = "";
      const urlToken = () =>
        btoa(String.fromCharCode(...getRandomBytes(32)))
          .replace(/\+/g, "-")
          .replace(/\//g, "_")
          .replace(/=+$/, "");
      const verifier = provider === "claude" ? urlToken() : "";
      const pkce =
        provider === "claude"
          ? {
              verifier,
              state: urlToken(),
              challenge: (
                await digestStringAsync(
                  CryptoDigestAlgorithm.SHA256,
                  verifier,
                  { encoding: CryptoEncoding.BASE64 },
                )
              )
                .replace(/\+/g, "-")
                .replace(/\//g, "_")
                .replace(/=+$/, ""),
            }
          : undefined;
      current.signal.throwIfAborted();
      return startSubscriptionConnect(provider, fetch, current.signal, pkce);
    },
    onSettled: () => {
      starting.current = false;
    },
  });
  const session = start.data;
  const finish = async (
    credential: SubscriptionCredential,
    signal: AbortSignal,
  ) => {
    const current = controller.current;
    if (!current) throw new Error("Start sign-in again.");
    const models = await listSubscriptionModels(
      provider,
      credential,
      fetch,
      signal,
    );
    if (!models.length)
      throw new Error("No models are available for this account.");
    const saved = await readProviderSetup(account, "llm", provider);
    current.signal.throwIfAborted();
    signal.throwIfAborted();
    await saveSubscriptionCredential(
      account,
      provider,
      credential,
      current.signal,
    );
    current.signal.throwIfAborted();
    signal.throwIfAborted();
    await saveProviderSetup(account, "llm", {
      ...saved,
      model: models.includes(saved.model) ? saved.model : models[0],
    });
    current.signal.throwIfAborted();
    signal.throwIfAborted();
    await invalidate();
    onSaved();
    return true;
  };
  const complete = useMutation({
    gcTime: 0,
    mutationFn: async () => {
      const current = controller.current;
      if (session?.kind !== "code" || !current)
        throw new Error("Start Claude sign-in again.");
      const credential = await completeClaudeConnect(
        session,
        form.state.values.code,
        fetch,
        current.signal,
      );
      await finish(credential, current.signal);
      form.reset();
      code.value = "";
    },
    onSettled: () => {
      completing.current = false;
    },
  });
  const completeCode = () => {
    if (completing.current) return;
    completing.current = true;
    complete.mutate();
  };
  const poll = useQuery({
    queryKey: [
      "subscription-connect",
      account,
      provider,
      session?.kind === "device" ? session.deviceCode : null,
    ],
    enabled: session?.kind === "device" && !controller.current?.signal.aborted,
    queryFn: async ({ signal }) => {
      const current = controller.current;
      if (session?.kind !== "device" || !current)
        throw new Error("Start sign-in again.");
      current.signal.throwIfAborted();
      const credential = await pollSubscriptionConnect(
        provider,
        session,
        fetch,
        signal,
      );
      return credential ? finish(credential, signal) : false;
    },
    refetchInterval: (query) =>
      query.state.data === true || query.state.error
        ? false
        : session?.kind === "device"
          ? session.interval
          : false,
    refetchIntervalInBackground: true,
    refetchOnWindowFocus: false,
    staleTime: Infinity,
    gcTime: 0,
    retry: false,
  });
  const resetStart = start.reset;
  const resetComplete = complete.reset;
  useFocusEffect(
    useCallback(
      () => () => {
        cancel();
        resetStart();
        resetComplete();
        form.reset();
        code.value = "";
      },
      [cancel, resetStart, resetComplete, form, code],
    ),
  );
  const browser = useMutation({
    mutationFn: () => {
      if (!session) throw new Error("Start sign-in again.");
      return Linking.openURL(session.url);
    },
  });
  const disconnect = useMutation({
    scope: { id: `provider-settings:${account}:llm` },
    mutationFn: async () => {
      await queryClient.cancelQueries({
        queryKey: ["provider-models", account, "llm", provider],
      });
      await removeProviderKey(account, "llm", provider);
    },
    onSuccess: invalidate,
  });
  const error =
    start.error ||
    poll.error ||
    complete.error ||
    browser.error ||
    disconnect.error;
  const done = poll.data === true || complete.isSuccess;
  return (
    <Column spacing={12}>
      <Text>{`Use your ${name} subscription for summaries. Sign in once on this device.`}</Text>
      {session && !done && !poll.error ? (
        <>
          {session.kind === "device" ? (
            <Text>{`Enter this code in ${name}: ${session.userCode}`}</Text>
          ) : (
            <Text>
              Approve access in your browser, then paste the authorization code
              here.
            </Text>
          )}
          {provider === "chatgpt" && (
            <Text>
              Enable device code login in ChatGPT Settings → Security if
              prompted.
            </Text>
          )}
          <Button
            label={`Open ${name}`}
            disabled={browser.isPending}
            onPress={() => browser.mutate()}
          />
          {session.kind === "code" ? (
            <>
              <TextInput
                value={code}
                placeholder="Authorization code"
                autoCapitalize="none"
                autoCorrect={false}
                secureTextEntry
                editable={!complete.isPending}
                onChangeText={(value) => {
                  code.value = value;
                  form.setFieldValue("code", value);
                }}
                onSubmitEditing={completeCode}
              />
              <Button
                label={complete.isPending ? "Connecting…" : "Finish connecting"}
                disabled={complete.isPending}
                onPress={completeCode}
              />
            </>
          ) : (
            <Text>
              Waiting for approval… After approving, return here to finish
              connecting.
            </Text>
          )}
          <Button
            label="Cancel"
            variant="text"
            onPress={() => {
              cancel();
              start.reset();
              complete.reset();
              form.reset();
              code.value = "";
            }}
          />
        </>
      ) : (
        <Button
          label={
            start.isPending
              ? "Starting sign-in…"
              : connected
                ? `Reconnect ${name}`
                : `Connect ${name}`
          }
          disabled={
            start.isPending || complete.isPending || disconnect.isPending
          }
          onPress={() => {
            if (starting.current) return;
            starting.current = true;
            complete.reset();
            start.mutate();
          }}
        />
      )}
      {done && (
        <Text>{`${name} connected. Choose it as your summary provider above.`}</Text>
      )}
      {connected && (
        <Row>
          <Button
            label="Disconnect"
            variant="text"
            disabled={disconnect.isPending}
            onPress={() => {
              cancel();
              start.reset();
              complete.reset();
              disconnect.mutate();
            }}
          />
        </Row>
      )}
      {(error || verificationError) && (
        <Text textStyle={{ color: Colors.destructive }}>
          {error?.message ?? verificationError}
        </Text>
      )}
    </Column>
  );
}
