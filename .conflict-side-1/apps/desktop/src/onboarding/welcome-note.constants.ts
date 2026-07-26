export const WELCOME_NOTE_DEMO_URL = "https://anarlog.so/onboarding-demo/";
export const WELCOME_NOTE_TRACKING_ID = "anarlog-onboarding-demo-v1";
export const WELCOME_NOTE_COMPLETE_PATH = "/onboarding-demo/complete";

export function buildWelcomeNoteDemoUrl(meetingLink: string, port: number) {
  const url = new URL(meetingLink);
  url.searchParams.set(
    "completion_url",
    `http://127.0.0.1:${port}${WELCOME_NOTE_COMPLETE_PATH}`,
  );
  return url.toString();
}
