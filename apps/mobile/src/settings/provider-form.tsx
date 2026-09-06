import {
  Button,
  Column,
  FieldGroup,
  Icon,
  ListItem,
  Picker,
  Row,
  Spacer,
  Text,
  TextInput,
  useNativeState,
} from "@expo/ui";
import { useForm } from "@tanstack/react-form";
import {
  useMutation,
  useQueries,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { useFocusEffect, useRouter } from "expo-router";
import { useCallback, useRef, useState } from "react";
import { Platform } from "react-native";

import { ProRequiredError } from "@/auth/billing";
import { useAuth } from "@/auth/context";

import { SettingsPage } from "./components";
import { createProviderAutosave } from "./provider-autosave";
import {
  readProviderConfig,
  readProviderKey,
  readProviderSetup,
  removeProviderKey,
  saveProviderConfig,
  saveProviderSetup,
} from "./providers";
import {
  providersFor,
  type ProviderConfig,
  type ProviderKind,
} from "./providers-model";
import { useColors } from "./theme-provider";

export function ProviderSettings({ kind }: { kind: ProviderKind }) {
  const auth = useAuth();
  const account = auth.session?.user.id ?? null;
  const config = useQuery({
    queryKey: ["provider", account, kind],
    queryFn: () => readProviderConfig(account, kind),
  });
  return (
    <SettingsPage
      title={kind === "stt" ? "Transcription provider" : "Summary provider"}
    >
      {config.isPending ? (
        <Text>Loading…</Text>
      ) : config.error ? (
        <Button label="Try again" onPress={() => void config.refetch()} />
      ) : (
        <ProviderForm
          key={`${account}:${kind}`}
          kind={kind}
          account={account}
          config={config.data}
        />
      )}
    </SettingsPage>
  );
}

function ProviderForm({
  kind,
  account,
  config,
}: {
  kind: ProviderKind;
  account: string | null;
  config: ProviderConfig;
}) {
  const auth = useAuth();
  const router = useRouter();
  const queryClient = useQueryClient();
  const definitions = providersFor(kind).filter(({ id }) => id !== "anarlog");
  const setups = useQueries({
    queries: definitions.map(({ id }) => ({
      queryKey: ["provider-setup", account, kind, id],
      queryFn: async () => ({
        config: await readProviderSetup(account, kind, id),
        hasKey: Boolean(await readProviderKey(account, kind, id)),
      }),
    })),
  });
  const [expanded, setExpanded] = useState<string | null>(null);
  const select = useMutation({
    scope: { id: `provider-settings:${account}:${kind}` },
    mutationFn: async (provider: string) => {
      if (provider === "anarlog" && !auth.billing.isPro && !auth.bypass)
        throw new ProRequiredError();
      const setup = await readProviderSetup(account, kind, provider);
      await saveProviderConfig(account, kind, setup);
    },
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["provider", account, kind] }),
    onError: (error) => {
      if (error instanceof ProRequiredError) router.push("/settings/pro");
    },
  });
  return (
    <>
      <FieldGroup.Section>
        <Row alignment="center">
          <Text>Provider</Text>
          <Spacer />
          <Picker
            key={`${config.provider}:${select.status}`}
            selectedValue={config.provider}
            enabled={!select.isPending}
            onValueChange={(provider) => {
              setExpanded(provider === "anarlog" ? null : provider);
              select.mutate(provider);
            }}
            testID="active-provider"
          >
            {providersFor(kind).map((provider) => (
              <Picker.Item
                key={provider.id}
                value={provider.id}
                label={provider.name}
              />
            ))}
          </Picker>
        </Row>
        <Row alignment="center">
          <Text>Model</Text>
          <Spacer />
          <Text>
            {config.provider === "anarlog" ? "Automatic" : config.model}
          </Text>
        </Row>
        <FieldGroup.SectionFooter>
          <Text>
            {select.error instanceof Error
              ? select.error.message
              : config.provider === "anarlog"
                ? "Included with your Pro trial or subscription. No API key needed."
                : "Using your saved provider settings below."}
          </Text>
        </FieldGroup.SectionFooter>
      </FieldGroup.Section>
      <FieldGroup.Section title="Configure providers">
        {definitions.map((provider, index) => {
          const setup = setups[index];
          const active = config.provider === provider.id;
          const open = expanded === provider.id;
          return (
            <Column key={provider.id} spacing={16}>
              <ListItem
                onPress={() => setExpanded(open ? null : provider.id)}
                trailing={
                  <Icon
                    name={
                      open
                        ? Icon.select({
                            ios: "chevron.down",
                            android:
                              import("@expo/material-symbols/keyboard_arrow_down.xml"),
                          })
                        : Icon.select({
                            ios: "chevron.right",
                            android:
                              import("@expo/material-symbols/chevron_right.xml"),
                          })
                    }
                    size={14}
                  />
                }
              >
                <Text>{`${provider.name}${active ? " · Active" : setup.data?.hasKey ? " · Key saved" : ""}`}</Text>
              </ListItem>
              {setup.isPending ? (
                open && <Text>Loading…</Text>
              ) : setup.error ? (
                open && (
                  <Button
                    label="Try again"
                    onPress={() => void setup.refetch()}
                  />
                )
              ) : (
                <ProviderFields
                  key={provider.id}
                  account={account}
                  kind={kind}
                  config={setup.data.config}
                  hasKey={setup.data.hasKey}
                  open={open}
                  onSaved={() => select.reset()}
                />
              )}
            </Column>
          );
        })}
        <FieldGroup.SectionFooter>
          <Text>
            Valid settings save automatically. API keys stay on this device.
          </Text>
        </FieldGroup.SectionFooter>
      </FieldGroup.Section>
    </>
  );
}

function ProviderFields({
  kind,
  account,
  config,
  hasKey,
  open,
  onSaved,
}: {
  kind: ProviderKind;
  account: string | null;
  config: ProviderConfig;
  hasKey: boolean;
  open: boolean;
  onSaved: () => void;
}) {
  const Colors = useColors();
  const queryClient = useQueryClient();
  const key = useNativeState("");
  const apiKey = useRef("");
  const model = useNativeState(config.model);
  const baseUrl = useNativeState(config.baseUrl);
  const invalidate = async () => {
    await queryClient.invalidateQueries({
      queryKey: ["provider-setup", account, kind, config.provider],
    });
    await queryClient.invalidateQueries({
      queryKey: ["provider", account, kind],
    });
  };
  const save = useMutation({
    gcTime: 0,
    scope: { id: `provider-settings:${account}:${kind}` },
    mutationFn: (draft: { config: ProviderConfig; apiKey: string }) =>
      saveProviderSetup(account, kind, draft.config, draft.apiKey),
    onSuccess: async () => {
      await invalidate();
      onSaved();
    },
  });
  const [autosave] = useState(() => createProviderAutosave(kind, save.mutate));
  useFocusEffect(useCallback(() => () => autosave.flush(), [autosave]));
  const remove = useMutation({
    scope: { id: `provider-settings:${account}:${kind}` },
    mutationFn: () => removeProviderKey(account, kind, config.provider),
    onSuccess: async () => {
      save.reset();
      await invalidate();
    },
  });
  const form = useForm({
    defaultValues: { model: config.model, baseUrl: config.baseUrl },
  });
  const scheduleSave = () => {
    if (remove.isPending) return;
    autosave.schedule(
      { ...config, ...form.state.values },
      apiKey.current,
      hasKey,
    );
  };
  if (!open) return null;
  return (
    <Column
      spacing={16}
      style={{
        paddingBottom: 8,
        paddingHorizontal: Platform.OS === "android" ? 16 : 0,
      }}
    >
      <Row alignment="center" spacing={8}>
        <Icon
          name={Icon.select({
            ios: "key",
            android: import("@expo/material-symbols/key.xml"),
          })}
          size={18}
          color={Colors.muted}
          style={{ width: 20 }}
        />
        <TextInput
          value={key}
          placeholder={hasKey ? "API key saved · enter to replace" : "API key"}
          secureTextEntry
          autoCapitalize="none"
          autoCorrect={false}
          editable={!remove.isPending}
          onBlur={autosave.flush}
          onSubmitEditing={autosave.flush}
          onChangeText={(value) => {
            key.value = value;
            apiKey.current = value;
            scheduleSave();
          }}
        />
      </Row>
      <Row alignment="center" spacing={8}>
        <Icon
          name={Icon.select({
            ios: "cpu",
            android: import("@expo/material-symbols/memory.xml"),
          })}
          size={18}
          color={Colors.muted}
          style={{ width: 20 }}
        />
        <TextInput
          value={model}
          placeholder="Model ID"
          autoCapitalize="none"
          autoCorrect={false}
          editable={!remove.isPending}
          onBlur={autosave.flush}
          onSubmitEditing={autosave.flush}
          onChangeText={(value) => {
            model.value = value;
            form.setFieldValue("model", value);
            scheduleSave();
          }}
        />
      </Row>
      {config.provider === "custom" && (
        <Row alignment="center" spacing={8}>
          <Icon
            name={Icon.select({
              ios: "link",
              android: import("@expo/material-symbols/link.xml"),
            })}
            size={18}
            color={Colors.muted}
            style={{ width: 20 }}
          />
          <TextInput
            value={baseUrl}
            placeholder="HTTPS base URL"
            autoCapitalize="none"
            autoCorrect={false}
            editable={!remove.isPending}
            onBlur={autosave.flush}
            onSubmitEditing={autosave.flush}
            onChangeText={(value) => {
              baseUrl.value = value;
              form.setFieldValue("baseUrl", value);
              scheduleSave();
            }}
          />
        </Row>
      )}
      {hasKey && (
        <Row>
          <Button
            label="Remove key"
            variant="text"
            disabled={remove.isPending}
            onPress={() => {
              autosave.cancel();
              key.value = "";
              apiKey.current = "";
              remove.mutate();
            }}
          />
        </Row>
      )}
      {(save.error || remove.error) && (
        <Text>{(save.error || remove.error)?.message}</Text>
      )}
      {kind === "stt" && (
        <Text>{`Transcription starts after recording.${config.provider === "deepgram" ? "" : " Recordings must be under 25 MB."}`}</Text>
      )}
    </Column>
  );
}
