import {
  Button,
  Column,
  FieldGroup,
  Icon,
  ListItem,
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
import { ProviderIcon } from "./provider-icon";
import { ProviderModelPicker } from "./provider-model-picker";
import { ProviderPicker } from "./provider-picker";
import {
  readProviderConfig,
  readProviderSetup,
  readProviderStatus,
  removeProviderKey,
  saveProviderConfig,
  saveProviderConnection,
  saveProviderSetup,
} from "./providers";
import {
  defaultProviderConfig,
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
      queryFn: () => readProviderStatus(account, kind, id),
    })),
  });
  const [expanded, setExpanded] = useState<string | null>(null);
  const [selectedProvider, setSelectedProvider] = useState(config.provider);
  const selectedProviderRef = useRef(config.provider);
  const modelDrafts = useRef<Record<string, string>>({});
  const selectedSetup =
    setups[definitions.findIndex(({ id }) => id === selectedProvider)];
  const configuredIds = new Set(
    setups.flatMap(({ data }) =>
      data?.isConfigured ? [data.config.provider] : [],
    ),
  );
  const providerOptions = providersFor(kind).filter(
    ({ id }) => id === "anarlog" || configuredIds.has(id),
  );
  const visibleProvider = providerOptions.some(
    ({ id }) => id === selectedProvider,
  )
    ? selectedProvider
    : "";
  const select = useMutation({
    scope: { id: `provider-settings:${account}:${kind}` },
    mutationFn: async ({
      provider,
      model,
    }: {
      provider: string;
      model?: string;
    }) => {
      if (provider === "anarlog" && !auth.billing.isPro && !auth.bypass)
        throw new ProRequiredError();
      const setup = await readProviderSetup(account, kind, provider);
      const save =
        selectedProviderRef.current === provider
          ? saveProviderConfig
          : saveProviderSetup;
      await save(account, kind, {
        ...setup,
        model: model?.trim() || setup.model,
      });
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["provider", account, kind],
      });
      await queryClient.invalidateQueries({
        queryKey: ["provider-setup", account, kind],
      });
    },
    onError: (error) => {
      if (error instanceof ProRequiredError) {
        selectedProviderRef.current = config.provider;
        setSelectedProvider(config.provider);
        router.push("/settings/pro");
      }
    },
  });
  return (
    <>
      <FieldGroup.Section>
        <Row alignment="center">
          <Text>Provider</Text>
          <Spacer />
          <ProviderPicker
            providers={providerOptions}
            selectedValue={visibleProvider}
            enabled={!select.isPending}
            onValueChange={(provider) => {
              if (!providerOptions.some(({ id }) => id === provider)) return;
              if (!modelDrafts.current[provider]?.trim())
                delete modelDrafts.current[provider];
              selectedProviderRef.current = provider;
              setSelectedProvider(provider);
              setExpanded(provider === "anarlog" ? null : provider);
              select.mutate({ provider, model: modelDrafts.current[provider] });
            }}
          />
        </Row>
        {!visibleProvider ? (
          <Text>Choose a configured provider to select a model.</Text>
        ) : selectedProvider === "anarlog" ? (
          <Row alignment="center">
            <Text>Model</Text>
            <Spacer />
            <Text>Automatic</Text>
          </Row>
        ) : selectedSetup?.data ? (
          <ProviderModelPicker
            key={selectedProvider}
            account={account}
            kind={kind}
            config={selectedSetup.data.config}
            model={
              modelDrafts.current[selectedProvider] ??
              selectedSetup.data.config.model
            }
            hasKey={selectedSetup.data.hasKey}
            onChange={(model) => {
              modelDrafts.current[selectedProvider] = model;
            }}
            onSave={(model) =>
              select.mutate({ provider: selectedProvider, model })
            }
          />
        ) : selectedSetup?.error ? (
          <Button
            label="Try again"
            onPress={() => void selectedSetup.refetch()}
          />
        ) : (
          <Text>Loading…</Text>
        )}
        <FieldGroup.SectionFooter>
          <Text>
            {select.error instanceof Error
              ? select.error.message
              : selectedProvider === "anarlog"
                ? "Included with your Pro trial or subscription. No API key needed."
                : "Choose your model here. Configure API keys and connections below."}
          </Text>
        </FieldGroup.SectionFooter>
      </FieldGroup.Section>
      <FieldGroup.Section title="Configure providers">
        {definitions.map((provider, index) => {
          const setup = setups[index];
          const active =
            config.provider === provider.id && setup.data?.isConfigured;
          const open = expanded === provider.id;
          return (
            <Column key={provider.id} spacing={16}>
              <ListItem
                leading={<ProviderIcon provider={provider.id} />}
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
                  onSaved={() => {
                    if (selectedProviderRef.current === provider.id) {
                      select.mutate({
                        provider: provider.id,
                        model: modelDrafts.current[provider.id],
                      });
                    }
                  }}
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
  const baseUrl = useNativeState(config.baseUrl);
  const invalidate = async () => {
    await queryClient.invalidateQueries({
      queryKey: ["provider-setup", account, kind, config.provider],
    });
    await queryClient.invalidateQueries({
      queryKey: ["provider", account, kind],
    });
    void queryClient.resetQueries({
      queryKey: ["provider-models", account, kind, config.provider],
    });
  };
  const save = useMutation({
    gcTime: 0,
    scope: { id: `provider-settings:${account}:${kind}` },
    mutationFn: async (draft: { config: ProviderConfig; apiKey: string }) => {
      await queryClient.cancelQueries({
        queryKey: ["provider-models", account, kind, config.provider],
      });
      await saveProviderConnection(account, kind, draft.config, draft.apiKey);
    },
    onSuccess: async () => {
      await invalidate();
      onSaved();
    },
  });
  const [autosave] = useState(() =>
    createProviderAutosave(kind, save.mutate, { connectionOnly: true }),
  );
  useFocusEffect(useCallback(() => () => autosave.flush(), [autosave]));
  const remove = useMutation({
    scope: { id: `provider-settings:${account}:${kind}` },
    mutationFn: async () => {
      await queryClient.cancelQueries({
        queryKey: ["provider-models", account, kind, config.provider],
      });
      await removeProviderKey(account, kind, config.provider);
    },
    onSuccess: async () => {
      save.reset();
      await invalidate();
    },
  });
  const form = useForm({
    defaultValues: { baseUrl: config.baseUrl },
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
      {!defaultProviderConfig(kind, config.provider).baseUrl && (
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
    </Column>
  );
}
