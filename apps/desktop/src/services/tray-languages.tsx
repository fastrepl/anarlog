import { useEffect } from "react";

import {
  commands as trayCommands,
  events as trayEvents,
  type TrayLanguageOption,
} from "@anlg/plugin-tray";
import { useMountEffect } from "@anlg/ui/hooks/use-mount-effect";

import {
  getBaseLanguageCode,
  getBaseLanguageDisplayName,
} from "~/settings/general/language";
import { getStoredSettingValues, setSettingValues } from "~/settings/queries";
import { resolveConfigValues, useConfigValues } from "~/shared/config";
import { getTranscriptionLanguages } from "~/stt/capabilities";

const LANGUAGE_KEYS = ["ai_language", "spoken_languages"] as const;

export function buildTrayLanguageOptions(
  mainLanguage: string | null | undefined,
  spokenLanguages: readonly string[] | null | undefined,
): TrayLanguageOption[] {
  return getTranscriptionLanguages(mainLanguage, spokenLanguages).map(
    (code, index) => ({
      code,
      label: getBaseLanguageDisplayName(code),
      selected: index === 0,
    }),
  );
}

// The selected language becomes the main language and the previous main
// language joins the additional spoken languages, so the configured set
// stays the same and the tray keeps offering every language. Codes are kept
// as stored so a regional main language like en-US survives a round trip.
function getTrayLanguageSelection(
  code: string,
  mainLanguage: string | null | undefined,
  spokenLanguages: readonly string[] | null | undefined,
) {
  const selectedLanguage = getBaseLanguageCode(code);

  return {
    ai_language: code,
    spoken_languages: JSON.stringify(
      getTranscriptionLanguages(mainLanguage, spokenLanguages).filter(
        (language) => getBaseLanguageCode(language) !== selectedLanguage,
      ),
    ),
  };
}

// Selections read settings before writing them, so run them one at a time;
// otherwise a quick second click can read the state from before the first.
let selectionQueue = Promise.resolve();

export function selectTrayLanguage(code: string): Promise<void> {
  const selection = selectionQueue.then(() => applyTrayLanguage(code));
  selectionQueue = selection.catch(() => {});
  return selection;
}

async function applyTrayLanguage(code: string) {
  const current = resolveConfigValues(
    LANGUAGE_KEYS,
    await getStoredSettingValues(),
  );
  const known = getTranscriptionLanguages(
    current.ai_language,
    current.spoken_languages,
  );
  if (!known.includes(code) || known[0] === code) {
    return;
  }

  await setSettingValues(
    getTrayLanguageSelection(
      code,
      current.ai_language,
      current.spoken_languages,
    ),
  );
}

export function TrayLanguagesSync() {
  const { ai_language, spoken_languages } = useConfigValues(LANGUAGE_KEYS);
  const options = buildTrayLanguageOptions(ai_language, spoken_languages);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void trayEvents.trayLanguageSelected
      .listen(({ payload }) => {
        void selectTrayLanguage(payload.code).catch((error) => {
          console.error("[tray] failed to switch language", error);
        });
      })
      .then((stop) => {
        if (disposed) {
          stop();
        } else {
          unlisten = stop;
        }
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  return (
    <TrayLanguagesPublisher key={JSON.stringify(options)} options={options} />
  );
}

// Tray commands run concurrently, so publish in order to keep an earlier
// list (such as the defaults rendered before settings load) from landing last.
let publishQueue = Promise.resolve();

function TrayLanguagesPublisher({
  options,
}: {
  options: TrayLanguageOption[];
}) {
  useMountEffect(() => {
    publishQueue = publishQueue.then(() => publishTrayLanguages(options));
  });

  return null;
}

async function publishTrayLanguages(options: TrayLanguageOption[]) {
  try {
    const result = await trayCommands.setTrayLanguages(options);
    if (result.status === "error") {
      console.error("[tray] failed to publish languages", result.error);
    }
  } catch (error) {
    console.error("[tray] failed to publish languages", error);
  }
}
