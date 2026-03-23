import { CheckIcon, Loader2Icon } from "lucide-react";

import { type ImportSourceInfo } from "@hypr/plugin-importer";
import { commands as openerCommands } from "@hypr/plugin-opener2";
import { Button } from "@hypr/ui/components/ui/button";

export function SourceItem({
  source,
  onScan,
  disabled,
  isScanning,
  isSuccess,
}: {
  source: ImportSourceInfo;
  onScan: () => void;
  disabled: boolean;
  isScanning: boolean;
  isSuccess?: boolean;
}) {
  return (
    <div className="flex items-center justify-between gap-4">
      <div className="min-w-0 flex-1">
        <h3 className="mb-1 text-sm font-medium">{source.name}</h3>
        <p className="text-xs text-neutral-600">
          Import data from `
          <button
            type="button"
            onClick={() => openerCommands.revealItemInDir(source.revealPath)}
            className="cursor-pointer underline hover:text-neutral-900"
          >
            {source.path}
          </button>
          `
        </p>
      </div>
      <div className="flex shrink-0 items-center gap-2">
        <Button
          size="sm"
          variant="outline"
          onClick={onScan}
          disabled={disabled || isSuccess}
        >
          {isScanning ? (
            <>
              <Loader2Icon size={14} className="mr-1 animate-spin" />
              Scanning...
            </>
          ) : isSuccess ? (
            <>
              <CheckIcon size={14} className="mr-1 text-green-600" />
            </>
          ) : (
            "Scan"
          )}
        </Button>
      </div>
    </div>
  );
}
