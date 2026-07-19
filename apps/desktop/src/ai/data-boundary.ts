export type GoogleCalendarDataState =
  | "loading"
  | "error"
  | "present"
  | "absent";

export type GoogleCalendarLlmBoundary =
  | "allowed"
  | "checking"
  | "check_failed"
  | "blocked";

const LOCAL_LLM_PROVIDERS = new Set(["lmstudio", "ollama"]);

export function isOnDeviceLlmConnection({
  providerId,
  baseUrl,
}: {
  providerId: string;
  baseUrl: string;
}): boolean {
  if (!LOCAL_LLM_PROVIDERS.has(providerId)) {
    return false;
  }

  return isLoopbackHttpUrl(baseUrl);
}

export function resolveGoogleCalendarLlmBoundary({
  googleCalendarDataState,
  providerId,
  baseUrl,
}: {
  googleCalendarDataState: GoogleCalendarDataState;
  providerId: string;
  baseUrl: string;
}): GoogleCalendarLlmBoundary {
  if (isOnDeviceLlmConnection({ providerId, baseUrl })) {
    return "allowed";
  }

  switch (googleCalendarDataState) {
    case "absent":
      return "allowed";
    case "loading":
      return "checking";
    case "error":
      return "check_failed";
    case "present":
      return "blocked";
  }
}

export function isOnDeviceSttTarget({
  provider,
  baseUrl,
}: {
  provider: string;
  baseUrl: string;
}): boolean {
  if (provider === "hyprnote") {
    return false;
  }

  if (provider !== "soniqo" && provider !== "am") {
    return false;
  }

  return baseUrl === "soniqo://local" || isLoopbackHttpUrl(baseUrl);
}

export function isOnDeviceLiveSttConnection({
  isLocalModel,
  baseUrl,
}: {
  isLocalModel: boolean;
  baseUrl: string | undefined;
}): boolean {
  return isLocalModel && !!baseUrl && isLoopbackHttpUrl(baseUrl);
}

export async function canSendCalendarDerivedSttHints({
  targetIsOnDevice,
  googleCalendarDataState,
  checkHasGoogleCalendarData,
}: {
  targetIsOnDevice: boolean;
  googleCalendarDataState: GoogleCalendarDataState;
  checkHasGoogleCalendarData: () => Promise<boolean>;
}): Promise<boolean> {
  if (targetIsOnDevice) {
    return true;
  }

  if (googleCalendarDataState !== "absent") {
    return false;
  }

  try {
    return !(await checkHasGoogleCalendarData());
  } catch {
    return false;
  }
}

export function isLoopbackHttpUrl(baseUrl: string): boolean {
  try {
    const url = new URL(baseUrl);
    const hostname = url.hostname.toLowerCase();
    return (
      (url.protocol === "http:" || url.protocol === "https:") &&
      (hostname === "localhost" ||
        hostname === "127.0.0.1" ||
        hostname === "[::1]")
    );
  } catch {
    return false;
  }
}
