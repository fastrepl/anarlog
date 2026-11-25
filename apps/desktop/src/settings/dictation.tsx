import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery } from "@tanstack/react-query";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { useShallow } from "zustand/react/shallow";

import { commands as transcription } from "@anlg/plugin-transcription";
import { Button } from "@anlg/ui/components/ui/button";
import { Textarea } from "@anlg/ui/components/ui/textarea";

import { DictationEngineSettings } from "./dictation-engine";
import { DictationShortcut } from "./dictation-shortcut";

import { useBillingAccess } from "~/auth/billing-context";
import { useDictationStatus } from "~/dictation/state";
import { AudioDeviceRow } from "~/settings/general/audio-settings";
import { SettingsPageTitle } from "~/settings/page-title";
import { PlanGate } from "~/settings/plan-gate";
import { useSetSettingValue } from "~/settings/queries";
import { SettingSwitchRow } from "~/settings/setting-row";
import { useConfigValue } from "~/shared/config";
import { useTabs } from "~/store/zustand/tabs";

export function SettingsDictation() {
  const { t } = useLingui();
  const { isPro } = useBillingAccess();
  const enabled = useConfigValue("dictation_enabled");
  const shortcut = useConfigValue("dictation_shortcut");
  const handsFree = useConfigValue("dictation_hands_free");
  const livePreview = useConfigValue("dictation_live_preview");
  const setLivePreview = useSetSettingValue("dictation_live_preview");
  const setEnabled = useSetSettingValue("dictation_enabled");
  const setHandsFree = useSetSettingValue("dictation_hands_free");
  const microphone = useConfigValue("microphone_device");
  const setMicrophone = useSetSettingValue("microphone_device");
  const microphones = useQuery({
    queryKey: ["microphone-devices"],
    queryFn: async () => {
      const result = await transcription.listMicrophoneDevices();
      if (result.status === "error") throw new Error(result.error);
      return result.data;
    },
    enabled: isPro,
    refetchInterval: 3_000,
  });
  const status = useDictationStatus(
    useShallow((state) => ({
      phase: state.phase,
      error: state.error,
      ready: state.ready,
      retry: state.retry,
      cancel: state.cancel,
      lastTranscript: state.lastTranscript,
    })),
  );
  const openNew = useTabs((state) => state.openNew);
  const copy = useMutation({
    mutationFn: (transcript: string) => writeText(transcript),
  });

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title={<Trans>Dictation</Trans>} />
      <PlanGate plan="pro" allowed={isPro}>
        <div className="flex flex-col gap-6">
          <SettingSwitchRow
            title={<Trans>Enable dictation</Trans>}
            checked={enabled}
            onChange={setEnabled}
          />
          <DictationShortcut shortcut={shortcut} />
          <AudioDeviceRow
            title={<Trans>Microphone</Trans>}
            value={microphone}
            devices={microphones.data ?? []}
            onChange={setMicrophone}
          />
          <SettingSwitchRow
            title={<Trans>Live transcript preview</Trans>}
            checked={livePreview}
            onChange={setLivePreview}
          />
          <SettingSwitchRow
            title={<Trans>Hands-free dictation</Trans>}
            checked={handsFree}
            onChange={setHandsFree}
          />
          <DictationEngineSettings />
          <div className="flex flex-wrap gap-2">
            <Button
              variant="outline"
              onClick={() =>
                openNew({ type: "settings", state: { tab: "permissions" } })
              }
            >
              <Trans>Permissions</Trans>
            </Button>
            <Button
              variant="outline"
              onClick={() =>
                openNew({ type: "settings", state: { tab: "transcription" } })
              }
            >
              <Trans>Language & transcription</Trans>
            </Button>
            <Button
              variant="outline"
              onClick={() =>
                openNew({ type: "settings", state: { tab: "dictionary" } })
              }
            >
              <Trans>Dictionary</Trans>
            </Button>
          </div>
          {enabled && (
            <div className="border-border rounded-lg border p-4" role="status">
              <p className="text-sm">
                {status.phase === "recording"
                  ? t`Listening…`
                  : status.phase === "transcribing"
                    ? t`Transcribing…`
                    : status.phase === "starting"
                      ? t`Starting microphone…`
                      : status.ready
                        ? t`Ready to dictate`
                        : t`Setting up dictation…`}
              </p>
              {status.error && (
                <p className="text-destructive mt-2 text-sm" role="alert">
                  {status.error}
                </p>
              )}
              {status.error && (
                <Button
                  className="mt-2"
                  variant="outline"
                  onClick={() =>
                    useDictationStatus.setState({ retry: status.retry + 1 })
                  }
                >
                  <Trans>Retry setup</Trans>
                </Button>
              )}
              {status.phase !== "idle" && (
                <Button
                  className="mt-2"
                  variant="outline"
                  onClick={() => status.cancel?.()}
                >
                  <Trans>Cancel dictation</Trans>
                </Button>
              )}
            </div>
          )}
          <label className="flex flex-col gap-2 text-sm">
            <span>
              <Trans>Try dictation</Trans>
            </span>
            <Textarea
              placeholder={t`Click here, then use your dictation shortcut.`}
            />
          </label>
          {status.lastTranscript && (
            <div className="flex flex-col gap-3">
              <h3 className="text-sm font-medium">
                <Trans>Last dictation</Trans>
              </h3>
              <p className="border-border rounded-lg border p-4 text-sm whitespace-pre-wrap">
                {status.lastTranscript}
              </p>
              <Button
                className="self-start"
                variant="outline"
                disabled={copy.isPending}
                onClick={() => copy.mutate(status.lastTranscript)}
              >
                {copy.isSuccess && copy.variables === status.lastTranscript ? (
                  <Trans>Copied</Trans>
                ) : (
                  <Trans>Copy last dictation</Trans>
                )}
              </Button>
              {copy.isError && (
                <p role="alert" className="text-destructive text-sm">
                  <Trans>
                    Could not copy the transcript. Select the text above to copy
                    it.
                  </Trans>
                </p>
              )}
            </div>
          )}
        </div>
      </PlanGate>
    </div>
  );
}
