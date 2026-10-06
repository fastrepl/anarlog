import { Column, Row } from "@expo/ui";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useFocusEffect } from "expo-router";
import { fetch } from "expo/fetch";
import { useCallback, useRef } from "react";
import { Linking } from "react-native";

import { saveChatgptCredential } from "./chatgpt-access";
import {
  CHATGPT_DEVICE_URL,
  listChatgptModels,
  pollChatgptDeviceCode,
  requestChatgptDeviceCode,
} from "./chatgpt-oauth";
import { Button, Text } from "./fields";
import {
  readProviderSetup,
  removeProviderKey,
  saveProviderSetup,
} from "./providers";
import { useColors } from "./theme-provider";

export function ChatgptConnect({
  account,
  connected,
  verificationError,
  onSaved,
}: {
  account: string | null;
  connected: boolean;
  verificationError?: string;
  onSaved: () => void;
}) {
  const Colors = useColors();
  const queryClient = useQueryClient();
  const controller = useRef<AbortController | null>(null);
  const starting = useRef(false);
  const cancel = useCallback(() => {
    controller.current?.abort();
    void queryClient.cancelQueries({ queryKey: ["chatgpt-connect", account] });
  }, [account, queryClient]);
  const invalidate = async () => {
    await queryClient.invalidateQueries({
      queryKey: ["provider-setup", account, "llm", "chatgpt"],
    });
    await queryClient.invalidateQueries({
      queryKey: ["provider", account, "llm"],
    });
    await queryClient.resetQueries({
      queryKey: ["provider-models", account, "llm", "chatgpt"],
    });
  };
  const start = useMutation({
    gcTime: 0,
    mutationFn: async () => {
      cancel();
      const current = new AbortController();
      controller.current = current;
      return requestChatgptDeviceCode(fetch, current.signal);
    },
    onSettled: () => {
      starting.current = false;
    },
  });
  const resetStart = start.reset;
  useFocusEffect(
    useCallback(
      () => () => {
        cancel();
        resetStart();
      },
      [cancel, resetStart],
    ),
  );
  const session = start.data;
  const poll = useQuery({
    queryKey: ["chatgpt-connect", account, session?.deviceAuthId],
    enabled: Boolean(session) && !controller.current?.signal.aborted,
    queryFn: async ({ signal }) => {
      const current = controller.current;
      if (!session || !current) throw new Error("Start ChatGPT sign-in again.");
      current.signal.throwIfAborted();
      const credential = await pollChatgptDeviceCode(session, fetch, signal);
      if (!credential) return false;
      const models = await listChatgptModels(
        credential.access,
        credential.accountId,
        fetch,
        signal,
      );
      if (!models.length)
        throw new Error("No ChatGPT models are available for this account.");
      const saved = await readProviderSetup(account, "llm", "chatgpt");
      current.signal.throwIfAborted();
      signal.throwIfAborted();
      await saveChatgptCredential(account, credential, current.signal);
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
    },
    refetchInterval: (query) =>
      query.state.data === true || query.state.error
        ? false
        : (session?.interval ?? false),
    refetchIntervalInBackground: true,
    refetchOnWindowFocus: false,
    staleTime: Infinity,
    gcTime: 0,
    retry: false,
  });
  const browser = useMutation({
    mutationFn: () => Linking.openURL(CHATGPT_DEVICE_URL),
  });
  const disconnect = useMutation({
    scope: { id: `provider-settings:${account}:llm` },
    mutationFn: async () => {
      await queryClient.cancelQueries({
        queryKey: ["provider-models", account, "llm", "chatgpt"],
      });
      await removeProviderKey(account, "llm", "chatgpt");
    },
    onSuccess: invalidate,
  });
  const error = start.error || poll.error || browser.error || disconnect.error;
  return (
    <Column spacing={12}>
      <Text>
        Use your ChatGPT subscription for summaries. Sign in once on this
        device.
      </Text>
      {session && poll.data !== true && !poll.error ? (
        <>
          <Text>{`Enter this code in ChatGPT: ${session.userCode}`}</Text>
          <Text>
            Enable device code login in ChatGPT Settings → Security if prompted.
            After approving, return here to finish connecting.
          </Text>
          <Button
            label="Open ChatGPT"
            disabled={browser.isPending}
            onPress={() => browser.mutate()}
          />
          <Text>Waiting for approval…</Text>
          <Button
            label="Cancel"
            variant="text"
            onPress={() => {
              cancel();
              start.reset();
            }}
          />
        </>
      ) : (
        <Button
          label={
            start.isPending
              ? "Starting sign-in…"
              : connected
                ? "Reconnect ChatGPT"
                : "Connect ChatGPT"
          }
          disabled={start.isPending || disconnect.isPending}
          onPress={() => {
            if (starting.current) return;
            starting.current = true;
            start.mutate();
          }}
        />
      )}
      {poll.data === true && (
        <Text>
          ChatGPT connected. Choose it as your summary provider above.
        </Text>
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
