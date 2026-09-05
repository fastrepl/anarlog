import {
  Button,
  FieldGroup,
  Picker,
  Row,
  Spacer,
  Text,
  TextInput,
  useNativeState,
} from "@expo/ui";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useRef, useState } from "react";

import { useAuth } from "@/auth/context";

import { SettingsPage } from "./components";
import {
  readProviderConfig,
  readProviderKey,
  removeProviderKey,
  saveProviderConfig,
} from "./providers";
import {
  defaultProviderConfig,
  providersFor,
  type ProviderConfig,
  type ProviderKind,
} from "./providers-model";

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
          key={`${account}:${kind}:${JSON.stringify(config.data)}`}
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
  const [selected, setSelected] = useState(config.provider);
  return (
    <>
      <FieldGroup.Section>
        <Row alignment="center">
          <Text>Provider</Text>
          <Spacer />
          <Picker selectedValue={selected} onValueChange={setSelected}>
            {providersFor(kind).map((provider) => (
              <Picker.Item
                key={provider.id}
                value={provider.id}
                label={provider.name}
              />
            ))}
          </Picker>
        </Row>
      </FieldGroup.Section>
      <ProviderFields
        key={selected}
        kind={kind}
        account={account}
        config={
          selected === config.provider
            ? config
            : defaultProviderConfig(kind, selected)
        }
      />
    </>
  );
}

function ProviderFields({
  kind,
  account,
  config,
}: {
  kind: ProviderKind;
  account: string | null;
  config: ProviderConfig;
}) {
  const queryClient = useQueryClient();
  const key = useNativeState("");
  const apiKey = useRef("");
  const model = useNativeState(config.model);
  const baseUrl = useNativeState(config.baseUrl);
  const savedKey = useQuery({
    queryKey: ["provider-key-present", account, kind, config.provider],
    queryFn: async () =>
      Boolean(await readProviderKey(account, kind, config.provider)),
  });
  const save = useMutation({
    mutationFn: (value: { model: string; baseUrl: string }) =>
      saveProviderConfig(
        account,
        kind,
        { ...config, model: value.model, baseUrl: value.baseUrl },
        apiKey.current,
      ),
    onSuccess: async () => {
      key.value = "";
      apiKey.current = "";
      await queryClient.invalidateQueries({
        queryKey: ["provider", account, kind],
      });
      await queryClient.invalidateQueries({
        queryKey: ["provider-key-present", account, kind],
      });
    },
  });
  const remove = useMutation({
    mutationFn: () => removeProviderKey(account, kind, config.provider),
    onSuccess: () => savedKey.refetch(),
  });
  const form = useForm({
    defaultValues: { model: config.model, baseUrl: config.baseUrl },
    onSubmit: async ({ value }) => {
      await save.mutateAsync(value);
    },
  });
  return (
    <>
      {config.provider !== "anarlog" && (
        <FieldGroup.Section>
          {config.provider === "custom" && (
            <TextInput
              value={baseUrl}
              placeholder="HTTPS base URL"
              autoCapitalize="none"
              autoCorrect={false}
              onChangeText={(value) => {
                baseUrl.value = value;
                form.setFieldValue("baseUrl", value);
              }}
            />
          )}
          <TextInput
            value={model}
            placeholder="Model ID"
            autoCapitalize="none"
            autoCorrect={false}
            onChangeText={(value) => {
              model.value = value;
              form.setFieldValue("model", value);
            }}
          />
          <TextInput
            value={key}
            placeholder={
              savedKey.data ? "API key saved · enter to replace" : "API key"
            }
            secureTextEntry
            autoCapitalize="none"
            autoCorrect={false}
            onChangeText={(value) => {
              key.value = value;
              apiKey.current = value;
            }}
          />
          <FieldGroup.SectionFooter>
            <Text>
              Keys are stored securely on this device. Requests go directly to
              the selected provider.
            </Text>
          </FieldGroup.SectionFooter>
        </FieldGroup.Section>
      )}
      <FieldGroup.Section>
        <Button
          label={
            save.isPending
              ? "Saving…"
              : save.isSuccess
                ? "Saved"
                : "Use this provider"
          }
          disabled={save.isPending}
          onPress={() => {
            void form.handleSubmit().catch(() => {});
          }}
        />
        {savedKey.data && (
          <Button
            label="Remove saved key"
            variant="text"
            disabled={remove.isPending}
            onPress={() => remove.mutate()}
          />
        )}
        <FieldGroup.SectionFooter>
          <Text>
            {save.error instanceof Error
              ? save.error.message
              : remove.error || savedKey.error
                ? "Could not update this provider. Try again."
                : config.provider === "anarlog"
                  ? "Included with your Anarlog Pro plan. No API key needed."
                  : kind === "stt"
                    ? `Transcription starts after you stop recording.${config.provider === "deepgram" ? "" : " Recordings must be under 25 MB."}`
                    : ""}
          </Text>
        </FieldGroup.SectionFooter>
      </FieldGroup.Section>
    </>
  );
}
