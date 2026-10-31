import { useMutation, useQuery } from "@tanstack/react-query";

import LocalSpeech from "../../modules/local-speech";
import { SettingsError, SettingsRow } from "./components";
import { FieldGroup } from "./field-group";
import { Text } from "./fields";

export function RecordingBatterySettings() {
  const status = useQuery({
    queryKey: ["recording-battery-exemption"],
    queryFn: () => LocalSpeech!.isIgnoringBatteryOptimizations(),
    refetchInterval: 2000,
  });
  const request = useMutation({
    mutationFn: () => LocalSpeech!.requestIgnoreBatteryOptimizations(),
    onSuccess: () => status.refetch(),
  });

  return (
    <FieldGroup.Section>
      <SettingsRow
        title="Battery optimization"
        value={
          status.isPending
            ? "Checking…"
            : status.isError
              ? "Could not check status"
              : status.data
                ? "Exempt"
                : "Allow unrestricted battery use"
        }
        onPress={
          status.data === false && !request.isPending
            ? () => request.mutate()
            : undefined
        }
      />
      <FieldGroup.SectionFooter>
        <Text>
          Exempt Anarlog from Android battery optimization to help keep
          recording and live transcription running with the screen locked. This
          may use more battery. Recording still works without this permission.
        </Text>
      </FieldGroup.SectionFooter>
      <SettingsError error={status.error || request.error} />
    </FieldGroup.Section>
  );
}
