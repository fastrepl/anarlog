import { CreditCardIcon } from "lucide-react";

import { cn } from "@hypr/utils";

import {
  ToolCard,
  ToolCardBody,
  ToolCardFooters,
  ToolCardHeader,
  useMcpOutput,
  useToolState,
} from "./shared";

import type { ToolRenderer } from "~/chat/components/message/types";
import { parseListSubscriptionsOutput } from "~/chat/mcp/support-mcp-tools";

type Renderer = ToolRenderer<"tool-list_subscriptions">;

function headerLabel(
  running: boolean,
  failed: boolean,
  statusFilter: string,
  parsed: ReturnType<typeof parseListSubscriptionsOutput>,
): string {
  if (running) return `Fetching subscriptions (${statusFilter})...`;
  if (failed) return "Subscription lookup failed";
  if (parsed)
    return `${parsed.length} subscription${parsed.length === 1 ? "" : "s"}`;
  return "List subscriptions";
}

function statusClass(status: string): string {
  const s = status.toLowerCase();
  if (s === "active")
    return "bg-emerald-50 text-emerald-700 border-emerald-200";
  if (s === "trialing") return "bg-blue-50 text-blue-700 border-blue-200";
  if (s === "past_due") return "bg-amber-50 text-amber-700 border-amber-200";
  if (s === "canceled" || s === "ended")
    return "bg-neutral-100 text-neutral-500 border-neutral-300";
  return "bg-neutral-100 text-neutral-700 border-neutral-300";
}

function formatTimestamp(value: number | null): string {
  if (!value) return "-";
  return new Date(value * 1000).toLocaleDateString();
}

export const ToolListSubscriptions: Renderer = ({ part }) => {
  const { running, failed, done } = useToolState(part);
  const { parsed, rawText } = useMcpOutput(
    done,
    part.output,
    parseListSubscriptionsOutput,
  );
  const statusFilter = part.input?.status ?? "active";

  return (
    <ToolCard failed={failed}>
      <ToolCardHeader
        icon={<CreditCardIcon />}
        running={running}
        failed={failed}
        done={!!parsed}
        label={headerLabel(running, failed, statusFilter, parsed)}
      />

      {parsed && parsed.length > 0 ? (
        <ToolCardBody>
          {parsed.map((sub) => (
            <div
              key={sub.id}
              className="flex flex-col gap-1 rounded-md border border-neutral-200 p-2"
            >
              <div className="flex items-center justify-between gap-2">
                <span className="truncate text-xs font-medium text-neutral-800">
                  {sub.id}
                </span>
                <span
                  className={cn([
                    "inline-flex shrink-0 items-center rounded-full border px-2 py-0 text-[10px] font-medium",
                    statusClass(sub.status),
                  ])}
                >
                  {sub.status}
                </span>
              </div>
              <p className="text-[11px] text-neutral-500">
                Start: {formatTimestamp(sub.start_date)} | Trial end:{" "}
                {formatTimestamp(sub.trial_end)}
              </p>
            </div>
          ))}
        </ToolCardBody>
      ) : null}

      {parsed && parsed.length === 0 ? (
        <ToolCardBody>
          <p className="py-1 text-center text-xs text-neutral-500">
            No subscriptions found
          </p>
        </ToolCardBody>
      ) : null}

      <ToolCardFooters
        failed={failed}
        errorText={part.errorText}
        rawText={rawText}
      />
    </ToolCard>
  );
};
