export const LOCAL_SPEECH_MODELS = [
  { id: "mlkit-advanced", name: "ML Kit Advanced" },
  { id: "mlkit-basic", name: "ML Kit Basic" },
  { id: "android-on-device", name: "Android on-device" },
] as const;

export type LocalSpeechModel = (typeof LOCAL_SPEECH_MODELS)[number]["id"];

export function localSpeechModel(value: string): LocalSpeechModel {
  const model = LOCAL_SPEECH_MODELS.find(({ id }) => id === value);
  if (!model) throw new Error("Choose a supported on-device speech engine.");
  return model.id;
}
