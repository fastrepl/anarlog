import { Trans, useLingui } from "@lingui/react/macro";
import { useMutation } from "@tanstack/react-query";
import { platform } from "@tauri-apps/plugin-os";
import { useEffect, useRef, useState } from "react";

import { commands as shortcuts } from "@anlg/plugin-shortcut";
import { Button } from "@anlg/ui/components/ui/button";

import { waitForDictationCleanup } from "~/dictation/lifecycle";
import { useDictationStatus } from "~/dictation/state";
import { setSettingValue } from "~/settings/queries";
import { useMountEffect } from "~/shared/hooks/useMountEffect";

export function DictationShortcut({ shortcut }: { shortcut: string }) {
  const { t } = useLingui();
  const mac = platform() === "macos";
  const [recording, setRecording] = useState(false);
  const attempt = useRef(0);
  const prepare = useMutation({ mutationFn: waitForDictationCleanup });
  useEffect(() => {
    if (recording) prepare.mutate();
  }, [recording, prepare.mutate]);
  const save = useMutation({
    mutationFn: async ({ value, token }: { value: string; token: number }) => {
      const result = await shortcuts.validate(value);
      if (token !== attempt.current) return;
      if (result.status === "error") throw new Error(result.error);
      await setSettingValue("dictation_shortcut", value);
    },
    onSuccess: (_, { token }) => {
      if (token === attempt.current) finish();
    },
  });

  function finish() {
    attempt.current += 1;
    setRecording(false);
    useDictationStatus.setState({ capturingShortcut: false });
    save.reset();
  }

  useMountEffect(() => () => {
    attempt.current += 1;
    useDictationStatus.setState({ capturingShortcut: false });
  });

  const labels: Record<string, string> = {
    Control: mac ? "⌃" : "Ctrl",
    Alt: mac ? "⌥" : "Alt",
    Shift: mac ? "⇧" : "Shift",
    Super: mac ? "⌘" : "Super",
    Meta: mac ? "⌘" : "Super",
    Fn: t`Fn / Globe`,
    RightCommand: t`Right Command`,
    ArrowLeft: "←",
    ArrowRight: "→",
    ArrowUp: "↑",
    ArrowDown: "↓",
  };

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-4">
        <span id="dictation-shortcut-label" className="text-sm font-medium">
          <Trans>Dictation shortcut</Trans>
        </span>
        <Button
          type="button"
          variant="outline"
          aria-labelledby="dictation-shortcut-label"
          aria-pressed={recording}
          onClick={() => {
            if (recording) return finish();
            attempt.current += 1;
            save.reset();
            useDictationStatus.setState({ capturingShortcut: true });
            prepare.reset();
            setRecording(true);
          }}
          onBlur={finish}
          onKeyDown={(event) => {
            if (!recording) return;
            event.stopPropagation();
            if (event.key === "Tab") {
              finish();
              return;
            }
            event.preventDefault();
            if (event.key === "Escape") return finish();
            if (event.repeat || !prepare.isSuccess || save.isPending) return;
            if (["Control", "Alt", "Meta", "Shift", "Fn"].includes(event.key))
              return;
            const modifiers = [
              event.ctrlKey && "Control",
              event.altKey && "Alt",
              event.shiftKey && "Shift",
              event.metaKey && "Super",
            ].filter(Boolean);
            save.mutate({
              value: [...modifiers, event.code].join("+"),
              token: attempt.current,
            });
          }}
        >
          {recording ? (
            !prepare.isSuccess ? (
              <Trans>Preparing…</Trans>
            ) : (
              <Trans>Press shortcut…</Trans>
            )
          ) : (
            <span className="inline-flex items-center gap-1">
              {shortcut.split("+").map((key) => (
                <kbd
                  key={key}
                  className="bg-muted rounded border px-1.5 py-0.5 font-sans text-xs"
                >
                  {labels[key] ?? key.replace(/^(Key|Digit)/, "")}
                </kbd>
              ))}
            </span>
          )}
        </Button>
      </div>
      <p className="text-muted-foreground text-xs">
        <Trans>
          Click the shortcut to record a new key combination. Press Escape to
          cancel.
        </Trans>
      </p>
      {save.error && (
        <p role="alert" className="text-destructive text-sm">
          {save.error.message}
        </p>
      )}
      {mac && (
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
              disabled={save.isPending}
              onClick={() => save.mutate({ value, token: attempt.current })}
            >
              {label}
            </Button>
          ))}
        </div>
      )}
    </div>
  );
}
