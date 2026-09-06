import {
  Button,
  Column,
  Icon,
  Picker,
  Row,
  Spacer,
  Text,
  TextInput,
  useNativeState,
} from "@expo/ui";
import { layoutPriority } from "@expo/ui/swift-ui/modifiers";
import { useForm } from "@tanstack/react-form";
import { useQuery } from "@tanstack/react-query";
import { useFocusEffect } from "expo-router";
import { useCallback, useState } from "react";
import { Platform } from "react-native";

import { createProviderAutosave } from "./provider-autosave";
import { modelOptions, presetProviderModels } from "./provider-model-catalog";
import { discoverProviderModels } from "./provider-models";
import type { ProviderConfig, ProviderKind } from "./providers-model";
import { useColors } from "./theme-provider";

export function ProviderModelPicker({
  kind,
  account,
  config,
  model: initialModel,
  hasKey,
  onChange,
  onSave,
}: {
  kind: ProviderKind;
  account: string | null;
  config: ProviderConfig;
  model: string;
  hasKey: boolean;
  onChange: (model: string) => void;
  onSave: (model: string) => void;
}) {
  const Colors = useColors();
  const input = useNativeState(initialModel);
  const [manual, setManual] = useState(false);
  const form = useForm({ defaultValues: { model: initialModel } });
  const presets = presetProviderModels(kind, config.provider);
  const models = useQuery({
    queryKey: [
      "provider-models",
      account,
      kind,
      config.provider,
      config.baseUrl,
    ],
    queryFn: ({ signal }) => discoverProviderModels(account, config, signal),
    enabled: !presets && hasKey && Boolean(config.baseUrl),
    staleTime: 5 * 60 * 1000,
    gcTime: 0,
    retry: false,
  });
  const [autosave] = useState(() =>
    createProviderAutosave(kind, ({ config }) => onSave(config.model)),
  );
  useFocusEffect(useCallback(() => () => autosave.flush(), [autosave]));
  const changeModel = (value: string) => {
    input.value = value;
    form.setFieldValue("model", value);
    onChange(value);
    autosave.schedule({ ...config, model: value }, "", hasKey);
  };
  return (
    <Column spacing={12}>
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
        <Text>Model</Text>
        <Spacer />
        <Column
          alignment="end"
          modifiers={Platform.OS === "ios" ? [layoutPriority(1)] : undefined}
        >
          <form.Subscribe selector={(state) => state.values.model}>
            {(model) => (
              <Picker
                selectedValue={
                  manual ? "manual" : model ? `model:${model}` : "empty"
                }
                onValueChange={(value) => {
                  if (value === "empty") return;
                  if (value === "manual") {
                    setManual(true);
                    return;
                  }
                  setManual(false);
                  changeModel(value.slice("model:".length));
                  autosave.flush();
                }}
                testID="provider-model"
              >
                {!model && <Picker.Item value="empty" label="Select model" />}
                {modelOptions(
                  presets ?? (hasKey ? (models.data ?? []) : []),
                  model,
                ).map((id) => (
                  <Picker.Item key={id} value={`model:${id}`} label={id} />
                ))}
                <Picker.Item value="manual" label="Enter model ID…" />
              </Picker>
            )}
          </form.Subscribe>
        </Column>
      </Row>
      {manual && (
        <TextInput
          value={input}
          placeholder="Model ID"
          autoCapitalize="none"
          autoCorrect={false}
          onBlur={autosave.flush}
          onSubmitEditing={autosave.flush}
          onChangeText={changeModel}
          testID="custom-model-id"
        />
      )}
      {!presets &&
        (!hasKey || !config.baseUrl ? (
          <Text textStyle={{ color: Colors.muted }}>
            Configure this provider below to load models.
          </Text>
        ) : models.isFetching ? (
          <Text textStyle={{ color: Colors.muted }}>Loading models…</Text>
        ) : models.error || !models.data?.length ? (
          <Column spacing={8}>
            <Text textStyle={{ color: Colors.muted }}>
              {models.error
                ? "Couldn’t load models. You can still enter a model ID."
                : "No models found. You can enter a model ID."}
            </Text>
            <Button label="Retry" onPress={() => void models.refetch()} />
          </Column>
        ) : null)}
    </Column>
  );
}
