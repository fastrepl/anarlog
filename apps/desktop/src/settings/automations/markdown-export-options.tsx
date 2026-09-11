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
              ["include_action_items", t`Collect action items`],
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
            <Trans>Needs setup</Trans>
          </p>
        )}
      </fieldset>
      <div className="flex flex-col gap-2">
        <label htmlFor={filenameId} className="text-xs font-medium">
          <Trans>Name</Trans>
        </label>
        <Input
          id={filenameId}
          value={options.filename}
          placeholder="{date} {title}.md"
          aria-describedby={helpId}
          className="h-8 text-xs"
          onChange={(event) =>
            onChange({ ...options, filename: event.target.value })
          }
        />
        <p id={helpId} className="text-muted-foreground text-xs">
          <Trans>
            Use a stable filename in the configured export directory.
          </Trans>
        </p>
        <dl className="text-muted-foreground flex flex-wrap gap-x-4 gap-y-1 text-xs">
          <div className="flex items-center gap-1.5">
            <dt>
              <code>{"{title}"}</code>
            </dt>
            <dd>
              <Trans>Session title</Trans>
            </dd>
          </div>
          <div className="flex items-center gap-1.5">
            <dt>
              <code>{"{date}"}</code>
            </dt>
            <dd>
              <Trans>Date</Trans>
            </dd>
          </div>
        </dl>
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
          <Trans>Include</Trans>
          {" ID "}
          <code className="text-muted-foreground">[a1b2c3d4]</code>
        </label>
      </div>
    </div>
  );
}
