import {
  Building2Icon,
  FileTextIcon,
  LayoutTemplateIcon,
  Loader2Icon,
  MicIcon,
  UserIcon,
  UsersIcon,
} from "lucide-react";

import { type ImportStats } from "@hypr/plugin-importer";
import { Button } from "@hypr/ui/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@hypr/ui/components/ui/tooltip";

export function ImportPreview({
  stats,
  sourceName,
  onConfirm,
  onCancel,
  isPending,
}: {
  stats: ImportStats;
  sourceName: string;
  onConfirm: () => void;
  onCancel: () => void;
  isPending: boolean;
}) {
  const totalItems =
    stats.sessionsCount +
    stats.transcriptsCount +
    stats.humansCount +
    stats.organizationsCount +
    stats.templatesCount;

  const hasData = totalItems > 0;

  const statItems = [
    { icon: FileTextIcon, label: "Sessions", count: stats.sessionsCount },
    { icon: MicIcon, label: "Transcripts", count: stats.transcriptsCount },
    { icon: UserIcon, label: "People", count: stats.humansCount },
    {
      icon: Building2Icon,
      label: "Organizations",
      count: stats.organizationsCount,
    },
    { icon: UsersIcon, label: "Participants", count: stats.participantsCount },
    {
      icon: LayoutTemplateIcon,
      label: "Templates",
      count: stats.templatesCount,
    },
  ];

  return (
    <div className="flex items-center justify-between gap-4">
      <div className="min-w-0 flex-1">
        <div className="mb-1 flex items-center gap-2">
          <h4 className="text-sm font-medium">{sourceName}</h4>
        </div>
        {hasData ? (
          <TooltipProvider delayDuration={100}>
            <div className="flex flex-wrap items-center gap-3 text-xs text-neutral-600">
              {statItems.map(({ icon: Icon, label, count }) => (
                <Tooltip key={label}>
                  <TooltipTrigger asChild>
                    <span className="flex items-center gap-1">
                      <Icon size={12} />
                      {count}
                    </span>
                  </TooltipTrigger>
                  <TooltipContent>{label}</TooltipContent>
                </Tooltip>
              ))}
            </div>
          </TooltipProvider>
        ) : (
          <p className="text-xs text-neutral-500">No data found to import.</p>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-2">
        <Button
          size="sm"
          variant="outline"
          onClick={onCancel}
          disabled={isPending}
        >
          Cancel
        </Button>
        {hasData && (
          <Button size="sm" onClick={onConfirm} disabled={isPending}>
            {isPending ? (
              <>
                <Loader2Icon size={14} className="mr-1 animate-spin" />
                Importing...
              </>
            ) : (
              `Import ${totalItems} items`
            )}
          </Button>
        )}
      </div>
    </div>
  );
}
