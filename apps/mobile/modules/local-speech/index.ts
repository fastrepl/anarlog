import { NativeModule, requireOptionalNativeModule } from "expo";

export type LocalSpeechStatus =
  | "available"
  | "downloadable"
  | "downloading"
  | "unavailable";

export type LocalSpeechEvent = {
  sessionId: string;
  type: "partial" | "final" | "completed" | "error";
  text?: string;
  startMs?: number;
  endMs?: number;
  code?: string;
};

declare class AnarlogLocalSpeechModule extends NativeModule<{
  onSpeechResult: (event: LocalSpeechEvent) => void;
}> {
  isIgnoringBatteryOptimizations(): Promise<boolean>;
  requestIgnoreBatteryOptimizations(): Promise<void>;
  checkStatus(locale: string, model: string): Promise<LocalSpeechStatus>;
  downloadModel(locale: string, model: string): Promise<void>;
  transcribeFile(uri: string, locale: string, model: string): Promise<string>;
  startLive(sessionId: string, locale: string, model: string): Promise<void>;
  sendAudio(sessionId: string, audio: Uint8Array): boolean;
  finishLive(sessionId: string): Promise<boolean>;
  cancelLive(sessionId: string): void;
}

export default requireOptionalNativeModule<AnarlogLocalSpeechModule>(
  "AnarlogLocalSpeech",
);
