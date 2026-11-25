import { Trans, useLingui } from "@lingui/react/macro";
import { platform } from "@tauri-apps/plugin-os";

import { Textarea } from "@anlg/ui/components/ui/textarea";

import { DictationRecoveryList } from "~/dictation/recovery";
import { parseContextExclusions } from "~/dictation/system";
import { useSetSettingValue } from "~/settings/queries";
import { SettingSwitchRow } from "~/settings/setting-row";
import { useConfigValue } from "~/shared/config";

/** Optional dictation features, each off until the person turns it on. */
export function DictationEngineSettings() {
  const cleanup = useConfigValue("dictation_cleanup");
  const setCleanup = useSetSettingValue("dictation_cleanup");
  const voiceEdit = useConfigValue("dictation_voice_edit");
  const setVoiceEdit = useSetSettingValue("dictation_voice_edit");
  const context = useConfigValue("dictation_context");
  const setContext = useSetSettingValue("dictation_context");
  const learning = useConfigValue("dictation_learning");
  const setLearning = useSetSettingValue("dictation_learning");
  const ducking = useConfigValue("dictation_ducking");
  const setDucking = useSetSettingValue("dictation_ducking");
  const recovery = useConfigValue("dictation_recovery");
  const setRecovery = useSetSettingValue("dictation_recovery");

  return (
    <div className="flex flex-col gap-6">
      <SettingSwitchRow
        title={<Trans>Clean up dictation</Trans>}
        checked={cleanup}
        onChange={setCleanup}
      />
      <SettingSwitchRow
        title={<Trans>Edit selected text by voice</Trans>}
        checked={voiceEdit}
        onChange={setVoiceEdit}
      />
      <SettingSwitchRow
        title={<Trans>Use what's on screen</Trans>}
        checked={context}
        onChange={setContext}
      />
      {context && <ContextExclusions />}
      <SettingSwitchRow
        title={<Trans>Learn from your corrections</Trans>}
        checked={learning}
        onChange={setLearning}
      />
      {platform() !== "linux" && (
        <SettingSwitchRow
          title={<Trans>Lower other audio while dictating</Trans>}
          checked={ducking}
          onChange={setDucking}
        />
      )}
      <SettingSwitchRow
        title={<Trans>Keep unsent dictations</Trans>}
        checked={recovery}
        onChange={setRecovery}
      />
      <DictationRecoveryList />
    </div>
  );
}

function ContextExclusions() {
  const { t } = useLingui();
  const stored = useConfigValue("dictation_context_exclusions");
  const setStored = useSetSettingValue("dictation_context_exclusions");
  const exclusions = parseContextExclusions(stored) ?? {
    apps: [],
    websites: [],
  };
  const save = (apps: string[], websites: string[]) =>
    setStored(JSON.stringify({ apps, websites }));

  return (
    <div className="flex flex-col gap-3">
      <label className="flex flex-col gap-2 text-sm">
        <span>
          <Trans>Never read these apps</Trans>
        </span>
        <Textarea
          defaultValue={exclusions.apps.join("\n")}
          placeholder={t`One app name or bundle ID per line`}
          onBlur={(event) =>
            save(lines(event.target.value), exclusions.websites)
          }
        />
      </label>
      <label className="flex flex-col gap-2 text-sm">
        <span>
          <Trans>Never read these websites</Trans>
        </span>
        <Textarea
          defaultValue={exclusions.websites.join("\n")}
          placeholder={t`One domain per line, like bank.example`}
          onBlur={(event) => save(exclusions.apps, lines(event.target.value))}
        />
      </label>
    </div>
  );
}

function lines(value: string) {
  return value
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .slice(0, 200);
}
