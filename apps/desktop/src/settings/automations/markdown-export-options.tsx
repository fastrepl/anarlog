import { Trans, useLingui } from "@lingui/react/macro";
import { useId } from "react";

import type { MarkdownExportOptions } from "@anlg/plugin-local-api";
import { Input } from "@anlg/ui/components/ui/input";

import { hasMarkdownExportContent } from "~/automations/markdown-export";

export function MarkdownExportOptionsConfig({
  options,
  onChange,
}: {
  options: MarkdownExportOptions;
  onChange: (options: MarkdownExportOptions) => void;
}) {
  const { t } = useLingui();
  const filenameId = useId();
  const helpId = useId();

  return (
    <div className="mt-4 flex flex-col gap-4">
      <fieldset className="flex flex-col gap-2">
        <legend className="mb-2 text-xs font-medium">
          <Trans>Include</Trans>
        </legend>
        <div className="flex flex-wrap gap-x-4 gap-y-2">
          {(
            [
              ["include_memo", t`Memo`],
              ["include_summary", t`Summary`],
              ["include_transcript", t`Transcript`],
              ["include_action_items", t`Action items`],
            ] as const
          ).map(([key, label]) => (
            <label
              key={key}
              className="flex cursor-pointer items-center gap-1.5 text-xs"
            >
              <input
                type="checkbox"
                className="accent-primary"
                checked={options[key]}
                onChange={(event) =>
                  onChange({ ...options, [key]: event.target.checked })
                }
              />
              {label}
            </label>
          ))}
        </div>
        {!hasMarkdownExportContent(options) && (
          <p role="alert" className="text-destructive text-xs">
            <Trans>Choose at least one element to export.</Trans>
          </p>
        )}
      </fieldset>
      <div className="flex flex-col gap-2">
        <label htmlFor={filenameId} className="text-xs font-medium">
          <Trans>Filename</Trans>
        </label>
        <Input
          id={filenameId}
          value={options.filename}
          placeholder={t`Meeting date and title`}
          aria-describedby={helpId}
          className="h-8 text-xs"
          onChange={(event) =>
            onChange({ ...options, filename: event.target.value })
          }
        />
        <p id={helpId} className="text-muted-foreground text-xs">
          <Trans>
            Leave blank to use the meeting date and title. Use {"{title}"} and{" "}
            {"{date}"} for a custom pattern. The .md extension is added
            automatically.
          </Trans>
        </p>
      </div>
      <div className="flex flex-col gap-2">
        <label className="flex cursor-pointer items-center gap-1.5 text-xs">
          <input
            type="checkbox"
            className="accent-primary"
            checked={options.include_id_suffix}
            onChange={(event) =>
              onChange({ ...options, include_id_suffix: event.target.checked })
            }
          />
          <Trans>Include meeting ID suffix</Trans>
        </label>
        <p className="text-muted-foreground text-xs">
          <Trans>
            Adds a short ID to distinguish meetings with the same filename. An
            existing file from another meeting will not be overwritten.
          </Trans>
        </p>
      </div>
    </div>
  );
}
