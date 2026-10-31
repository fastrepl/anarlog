import { Column } from "@expo/ui";
import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";

import { downloadLocalSpeechModel } from "@/data/local-speech";

import { Button, Picker, Text } from "./fields";
import { localSpeechModelOptions } from "./local-speech-model";
import { LOCAL_SPEECH_MODELS } from "./local-speech-models";
import { usePreferences } from "./preferences";
import { useColors } from "./theme-provider";

export function LocalSpeechSettings({
  model: savedModel,
  initialModel,
  saving,
  onSelect,
}: {
  model?: string;
  initialModel: string;
  saving: boolean;
  onSelect: (model: string) => Promise<void>;
}) {
  const [model, setModel] = useState(savedModel ?? initialModel);
  const Colors = useColors();
  const preferences = usePreferences();
  const status = useQuery({
    ...localSpeechModelOptions(preferences, model),
    refetchInterval: (query) =>
      query.state.data?.status === "downloading" ? 3000 : false,
  });
  const download = useMutation({
    mutationFn: async ({
      language,
      model,
    }: {
      language: string;
      model: string;
    }) => {
      await downloadLocalSpeechModel(language, model);
    },
    onSuccess: async (_, { model }) => {
      await status.refetch();
      await onSelect(model);
    },
  });
  return (
    <Column spacing={12}>
      <Picker
        selectedValue={model}
        enabled={!saving && !download.isPending}
        onValueChange={(value) => {
          setModel(value);
          download.reset();
          void onSelect(value).catch(() => {});
        }}
      >
        {LOCAL_SPEECH_MODELS.map(({ id, name }) => (
          <Picker.Item key={id} value={id} label={name} />
        ))}
      </Picker>
      {model !== savedModel && (
        <Text>
          Your current provider stays active until this model is ready.
        </Text>
      )}
      <Text>
        {`Active engine: ${LOCAL_SPEECH_MODELS.find(({ id }) => id === savedModel)?.name ?? "None"}`}
      </Text>
      <Text>
        Transcribes speech on this device while you record, including in the
        background when supported by your phone. If interrupted, the saved audio
        remains available for recovery.
      </Text>
      <Text>
        Uses your first spoken language, or your main language if none is
        selected. Supports Anarlog recordings and 16 kHz mono PCM WAV imports.
        Transcribing a saved recording takes about as long as the recording.
      </Text>
      <Text>
        {download.isPending
          ? "Downloading speech model. Keep Anarlog open…"
          : status.isPending
            ? "Checking speech model…"
            : status.data?.status === "available"
              ? "Speech model ready. Transcription works offline."
              : status.data?.status === "downloadable"
                ? "Download the speech model once using an internet connection."
                : status.data?.status === "downloading"
                  ? "Google is downloading the speech model…"
                  : status.data?.status === "unavailable"
                    ? "This engine or language is unavailable on this phone. Android on-device requires Android 13 or newer. Update Google's speech services and check again."
                    : "Could not check the speech model."}
      </Text>
      {(status.error || download.error) && (
        <Text textStyle={{ color: Colors.destructive }}>
          {(download.error ?? status.error)?.message}
        </Text>
      )}
      {status.data?.status === "downloadable" && (
        <Button
          label="Download speech model"
          disabled={download.isPending}
          onPress={() => {
            if (status.data)
              download.mutate({ language: status.data.language, model });
          }}
        />
      )}
      <Button
        label="Check again"
        variant="text"
        disabled={saving || download.isPending || status.isFetching}
        onPress={async () => {
          download.reset();
          await status.refetch();
          await onSelect(model).catch(() => {});
        }}
      />
    </Column>
  );
}
