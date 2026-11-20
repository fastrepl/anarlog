import { useRef } from "react";

import { commands as dictation } from "@anlg/plugin-dictation";
import { useMountEffect } from "@anlg/ui/hooks/use-mount-effect";

import { waitForDictationSetup } from "./serial";
import { useDictationStatus } from "./state";
import { SystemDictation } from "./system";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { useSettingsReady } from "~/settings/queries";
import { useConfigValue } from "~/shared/config";
import { useListener } from "~/stt/contexts";

export function waitForDictationCleanup() {
  return waitForDictationSetup();
}

export function DictationLifecycle() {
  const { session } = useAuth();
  const { isPro, isReady } = useBillingAccess();
  const settingsReady = useSettingsReady();
  const enabled = useConfigValue("dictation_enabled");
  const shortcut = useConfigValue("dictation_shortcut");
  const handsFree = useConfigValue("dictation_hands_free");
  const capturingShortcut = useDictationStatus(
    (state) => state.capturingShortcut,
  );
  const retry = useDictationStatus((state) => state.retry);
  const meetingActive = useListener(
    (state) => state.live.status !== "inactive" || state.live.loading,
  );
  const accountId = session?.user.id ?? null;
  const currentAccount = useRef(accountId);
  currentAccount.current = accountId;

  return (
    <>
      {accountId && (
        <AccountRetirement
          key={`account:${accountId}`}
          accountId={accountId}
          current={currentAccount}
        />
      )}
      {meetingActive && <MeetingRetirement />}
      {session && isReady && isPro && settingsReady && enabled && (
        <TranscriptRetention key={session.user.id}>
          {!meetingActive && !capturingShortcut && (
            <SystemDictation
              key={`${shortcut}:${handsFree}:${retry}`}
              shortcut={shortcut}
              handsFree={handsFree}
            />
          )}
        </TranscriptRetention>
      )}
    </>
  );
}

/**
 * Leaving an account (sign-out or switch) retires its dictations and recovery records, so
 * late results never land under, or upload for, the next account.
 */
function AccountRetirement({
  accountId,
  current,
}: {
  accountId: string;
  current: React.RefObject<string | null>;
}) {
  useMountEffect(() => () => {
    if (current.current === accountId) return;
    void retireDictations({ type: "account", accountId });
  });
  return null;
}

/** A meeting starting takes the microphone from any dictation, system or chat. */
function MeetingRetirement() {
  useMountEffect(() => {
    void retireDictations({ type: "meeting" });
  });
  return null;
}

async function retireDictations(
  reason: { type: "meeting" } | { type: "account"; accountId: string },
) {
  try {
    const result = await dictation.retireDictations(reason);
    if (result.status === "error") throw new Error(result.error.message);
  } catch (error) {
    console.error("[dictation] failed to retire dictations", error);
  }
}

function TranscriptRetention({ children }: { children: React.ReactNode }) {
  useMountEffect(() => () => {
    useDictationStatus.setState({ lastTranscript: "" });
  });
  return children;
}
