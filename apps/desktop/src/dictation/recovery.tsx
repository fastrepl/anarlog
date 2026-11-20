import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation, useQuery } from "@tanstack/react-query";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { useRef } from "react";

import {
  commands as dictation,
  type DictationRecoveryItem,
} from "@anlg/plugin-dictation";
import { Button } from "@anlg/ui/components/ui/button";

import { errorMessage } from "./engine";
import { unwrapDictation } from "./native";
import { useDictationPlanner } from "./planner";
import { useDictationStatus } from "./state";

import { useAuth } from "~/auth";

/**
 * Dictations kept on this device because they couldn't be transcribed or inserted. Nothing
 * here is inserted anywhere by itself: the person retries, copies, uses or discards each.
 */
export function DictationRecoveryList() {
  const { t } = useLingui();
  const { session } = useAuth();
  const accountId = session?.user.id ?? null;
  const currentAccount = useRef(accountId);
  currentAccount.current = accountId;
  const sessionId = useRef(`dictation-recovery-${crypto.randomUUID()}`).current;
  const planner = useDictationPlanner(sessionId);
  const version = useDictationStatus((state) => state.recoveryVersion);
  const items = useQuery({
    queryKey: ["dictation-recovery", accountId, version],
    enabled: !!accountId,
    queryFn: async () =>
      unwrapDictation(await dictation.listDictationRecovery(accountId!)),
  });
  const refresh = () =>
    useDictationStatus.setState((state) => ({
      recoveryVersion: state.recoveryVersion + 1,
    }));

  const action = useMutation({
    mutationFn: async ({
      item,
      kind,
    }: {
      item: DictationRecoveryItem;
      kind: "retry" | "copy" | "use" | "discard";
    }) => {
      if (!accountId) return;
      if (kind === "discard") {
        unwrapDictation(
          await dictation.discardDictationRecovery(accountId, item.id),
        );
        return;
      }
      if (kind === "copy") {
        if (item.text) await writeText(item.text);
        return;
      }
      if (kind === "use") {
        // Copy first, so the text is never lost if the clipboard refuses it.
        if (item.text) await writeText(item.text);
        unwrapDictation(
          await dictation.claimDictationRecovery(accountId, item.id),
        );
        return;
      }
      const plan = await planner.plan({ accountId, context: null });
      const audioPath = unwrapDictation(
        await dictation.prepareDictationRecoveryAudio(
          accountId,
          item.id,
          plan.policy,
        ),
      );
      const text = await plan.transcribe(
        audioPath,
        new AbortController().signal,
      );
      if (!text) throw new Error(t`No speech was detected in this recording.`);
      // A recording finished for one account is never saved under the next.
      if (currentAccount.current !== accountId) return;
      unwrapDictation(
        await dictation.saveDictationRecoveryText(accountId, item.id, text),
      );
    },
    onSettled: (_data, _error, variables) => {
      if (variables.kind !== "copy") refresh();
    },
  });

  if (!items.data?.length) return null;

  return (
    <div className="flex flex-col gap-3">
      <h3 className="text-sm font-medium">
        <Trans>Unsent dictations</Trans>
      </h3>
      <p className="text-muted-foreground text-xs">
        <Trans>
          Kept on this device for 24 hours. Anarlog never inserts these by
          itself.
        </Trans>
      </p>
      <div className="border-border divide-border divide-y rounded-lg border">
        {items.data.map((item) => (
          <div key={item.id} className="flex flex-col gap-2 p-4">
            <p className="text-muted-foreground text-xs">
              {new Date(item.createdAt).toLocaleString()} ·{" "}
              {item.surface === "chat" ? t`Chat` : t`Other apps`} ·{" "}
              {t`${seconds(item)} s`}
            </p>
            {item.text ? (
              <p className="text-sm whitespace-pre-wrap">{item.text}</p>
            ) : (
              <p className="text-sm">
                <Trans>Recording not transcribed yet.</Trans>
              </p>
            )}
            <div className="flex flex-wrap gap-2">
              {item.state === "audio" ? (
                <Button
                  variant="outline"
                  size="sm"
                  disabled={action.isPending}
                  onClick={() => action.mutate({ item, kind: "retry" })}
                >
                  <Trans>Transcribe</Trans>
                </Button>
              ) : (
                <>
                  <Button
                    variant="outline"
                    size="sm"
                    disabled={action.isPending}
                    onClick={() => action.mutate({ item, kind: "copy" })}
                  >
                    <Trans>Copy</Trans>
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    disabled={action.isPending}
                    onClick={() => action.mutate({ item, kind: "use" })}
                  >
                    <Trans>Copy and remove</Trans>
                  </Button>
                </>
              )}
              <Button
                variant="ghost"
                size="sm"
                disabled={action.isPending}
                onClick={() => action.mutate({ item, kind: "discard" })}
              >
                <Trans>Discard</Trans>
              </Button>
            </div>
          </div>
        ))}
      </div>
      {action.isError && (
        <p role="alert" className="text-destructive text-sm">
          {errorMessage(action.error)}
        </p>
      )}
    </div>
  );
}

function seconds(item: DictationRecoveryItem) {
  return Math.max(1, Math.round(item.durationMs / 1_000));
}
