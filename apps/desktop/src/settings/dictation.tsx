import { Trans, useLingui } from "@lingui/react/macro";
import { useForm } from "@tanstack/react-form";
import { useMutation, useQuery } from "@tanstack/react-query";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { platform } from "@tauri-apps/plugin-os";

import { commands as transcription } from "@anlg/plugin-transcription";
import { Button } from "@anlg/ui/components/ui/button";
import { Input } from "@anlg/ui/components/ui/input";
import { Textarea } from "@anlg/ui/components/ui/textarea";

import { useBillingAccess } from "~/auth/billing-context";
import { useDictationStatus } from "~/dictation/lifecycle";
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
  const status = useDictationStatus();
  const openNew = useTabs((state) => state.openNew);
  const copy = useMutation({
    mutationFn: () => writeText(status.lastTranscript),
  });

  return (
    <div className="flex flex-col gap-8">
      <SettingsPageTitle title={<Trans>Dictation</Trans>} />
      <PlanGate plan="pro" allowed={isPro}>
        <div className="flex flex-col gap-6">
          <p className="text-muted-foreground text-sm">
            <Trans>
              Speak into the focused text field in your apps. Your transcript
              appears when you finish.
            </Trans>
          </p>
          <SettingSwitchRow
            title={<Trans>Enable dictation</Trans>}
            description={
              <Trans>
                Keep Anarlog running to use your shortcut in other apps.
              </Trans>
            }
            checked={enabled}
            onChange={setEnabled}
          />
          <ShortcutSetting key={shortcut} shortcut={shortcut} />
          <AudioDeviceRow
            title={<Trans>Microphone</Trans>}
            description={
              <Trans>Choose the microphone that captures your voice.</Trans>
            }
            value={microphone}
            devices={microphones.data ?? []}
            onChange={setMicrophone}
          />
          <SettingSwitchRow
            title={<Trans>Hands-free dictation</Trans>}
            description={
              handsFree ? (
                <Trans>
                  Press your shortcut to start, then press it again to finish.
                </Trans>
              ) : (
                <Trans>
                  Hold your shortcut while speaking, then release it to finish.
                </Trans>
              )
            }
            checked={handsFree}
            onChange={setHandsFree}
          />
          <p className="text-muted-foreground text-xs">
            {platform() === "linux" ? (
              <Trans>
                On Wayland, approve the shortcuts in your desktop's prompt. Use
                Control + Alt + Escape to cancel. On X11, press Escape to
                cancel. The target app must expose an editable accessibility
                field.
              </Trans>
            ) : (
              <Trans>
                Press Escape to cancel. Dictation stops after five minutes and
                pauses while Anarlog records a meeting.
              </Trans>
            )}
          </p>
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
          <p className="text-muted-foreground text-xs">
            <Trans>
              Dictation uses your microphone, transcription provider, languages,
              and dictionary. Temporary audio is deleted after transcription.
              Your last transcript stays in memory until dictation is disabled
              or Anarlog closes.
            </Trans>
          </p>
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
                onClick={() => copy.mutate()}
              >
                {copy.isSuccess ? (
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

function ShortcutSetting({ shortcut }: { shortcut: string }) {
  const { t } = useLingui();
  const save = useSetSettingValue("dictation_shortcut");
  const form = useForm({
    defaultValues: { shortcut },
    onSubmit: ({ value }) => {
      save(value.shortcut.trim());
    },
  });
  return (
    <form
      className="flex flex-col gap-2"
      onSubmit={(event) => {
        event.preventDefault();
        void form.handleSubmit();
      }}
    >
      <form.Field name="shortcut">
        {(field) => (
          <>
            <label htmlFor="dictation-shortcut" className="text-sm font-medium">
              <Trans>Dictation shortcut</Trans>
            </label>
            <div className="flex gap-2">
              <Input
                id="dictation-shortcut"
                value={field.state.value}
                onChange={(event) => field.handleChange(event.target.value)}
                onKeyDown={(event) => {
                  if (
                    !(
                      event.ctrlKey ||
                      event.altKey ||
                      event.metaKey ||
                      event.shiftKey
                    ) ||
                    ["Control", "Alt", "Meta", "Shift"].includes(event.key)
                  )
                    return;
                  event.preventDefault();
                  const modifiers = [
                    event.ctrlKey && "Control",
                    event.altKey && "Alt",
                    event.shiftKey && "Shift",
                    event.metaKey && "Super",
                  ].filter(Boolean);
                  field.handleChange([...modifiers, event.code].join("+"));
                }}
                placeholder="Control+Alt+Space"
              />
              <Button
                type="submit"
                disabled={
                  !field.state.value.trim() || field.state.value === shortcut
                }
              >
                <Trans>Save</Trans>
              </Button>
            </div>
            <p className="text-muted-foreground text-xs">
              <Trans>
                Press a key combination here, or type one such as Control+Alt+D.
              </Trans>
            </p>
            {platform() === "macos" && (
              <div className="flex gap-2">
                {[
                  ["Fn", t`Fn / Globe`],
                  ["RightCommand", t`Right Command`],
                ].map(([value, label]) => (
                  <Button
                    key={value}
                    type="button"
                    size="sm"
                    variant="outline"
                    onClick={() => {
                      field.handleChange(value);
                      void form.handleSubmit();
                    }}
                  >
                    {label}
                  </Button>
                ))}
              </div>
            )}
          </>
        )}
      </form.Field>
    </form>
  );
}
