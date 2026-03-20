import {
  CheckCircle2Icon,
  ExternalLinkIcon,
  Loader2Icon,
  ShieldAlertIcon,
  XCircleIcon,
} from "lucide-react";
import type { ReactNode } from "react";
import { Streamdown } from "streamdown";

import { cn } from "@hypr/utils";

import { extractMcpOutputText } from "~/chat/mcp/mcp-output-parser";
import { useElicitation } from "~/contexts/elicitation";

export function ToolCard({
  failed,
  children,
}: {
  failed?: boolean;
  children: ReactNode;
}) {
  return (
    <div
      className={cn([
        "my-2.5 overflow-hidden rounded-xl border shadow-sm",
        failed ? "border-red-200" : "border-neutral-200/80",
      ])}
    >
      {children}
    </div>
  );
}

export function ToolCardHeader({
  icon,
  running,
  awaitingApproval,
  failed,
  done,
  label,
}: {
  icon: ReactNode;
  running: boolean;
  awaitingApproval?: boolean;
  failed: boolean;
  done: boolean;
  label: string;
}) {
  return (
    <div
      className={cn([
        "flex items-center gap-2.5 px-3.5 py-2 text-[13px]",
        failed
          ? "bg-red-50 text-red-700"
          : awaitingApproval
            ? "bg-neutral-100 text-neutral-700"
            : "bg-neutral-50/80 text-neutral-600",
      ])}
    >
      {running && !awaitingApproval ? (
        <Loader2Icon className="h-4 w-4 animate-spin" />
      ) : awaitingApproval ? (
        <ShieldAlertIcon className="h-4 w-4 text-neutral-500" />
      ) : (
        <span
          className={cn([
            "shrink-0 [&>svg]:h-4 [&>svg]:w-4",
            failed
              ? "text-red-500"
              : done
                ? "text-emerald-500"
                : "text-neutral-400",
          ])}
        >
          {icon}
        </span>
      )}
      <span className="font-medium">{label}</span>
    </div>
  );
}

export function ToolCardBody({ children }: { children: ReactNode }) {
  return <div className="flex flex-col gap-2.5 px-3.5 py-2.5">{children}</div>;
}

export function ToolCardFooterSuccess({
  href,
  label,
}: {
  href: string;
  label: string;
}) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noreferrer"
      className="flex items-center gap-2 border-t border-emerald-200 bg-emerald-50 px-3.5 py-2.5 transition-colors hover:bg-emerald-100/80"
    >
      <CheckCircle2Icon className="h-4 w-4 shrink-0 text-emerald-600" />
      <span className="text-[13px] font-medium text-emerald-700">{label}</span>
      <ExternalLinkIcon className="ml-auto h-3.5 w-3.5 shrink-0 text-emerald-500" />
    </a>
  );
}

export function ToolCardFooterError({ text }: { text: string }) {
  return (
    <div className="flex items-center gap-2 border-t border-red-200 bg-red-50 px-3.5 py-2.5">
      <XCircleIcon className="h-4 w-4 shrink-0 text-red-500" />
      <p className="text-[13px] text-red-600">{text}</p>
    </div>
  );
}

export function ToolCardFooterRaw({ text }: { text: string }) {
  return (
    <div className="border-t border-neutral-200/80 bg-neutral-50/80 px-3.5 py-2.5">
      <p className="text-[13px] whitespace-pre-wrap text-neutral-600">{text}</p>
    </div>
  );
}

export function useToolState(part: { state: string }) {
  const running =
    part.state === "input-streaming" || part.state === "input-available";
  const failed = part.state === "output-error";
  const done = part.state === "output-available";
  return { running, failed, done };
}

export function useMcpOutput<T>(
  done: boolean,
  output: unknown,
  parseFn: (output: unknown) => T | null,
): { parsed: T | null; rawText: string | null } {
  const parsed = done ? parseFn(output) : null;
  const rawText = done && !parsed ? extractMcpOutputText(output) : null;
  return { parsed, rawText };
}

export function ToolCardFooters({
  failed,
  errorText,
  rawText,
  children,
}: {
  failed: boolean;
  errorText?: unknown;
  rawText: string | null;
  children?: ReactNode;
}) {
  return (
    <>
      {children}
      {failed ? (
        <ToolCardFooterError text={String(errorText ?? "Unknown error")} />
      ) : null}
      {rawText ? <ToolCardFooterRaw text={rawText} /> : null}
    </>
  );
}

export function ToolCardApproval() {
  const { pending, respond } = useElicitation();

  if (!pending || !respond) {
    return null;
  }

  return (
    <div className="flex items-center gap-2.5 border-t border-neutral-200/80 bg-neutral-50/80 px-3.5 py-2.5">
      <span className="flex-1 text-[13px] text-neutral-500">
        {pending.message}
      </span>
      <button
        className="rounded-md border border-neutral-300 bg-white px-3 py-1 text-[13px] text-neutral-600 transition-colors hover:bg-neutral-50"
        onClick={() => respond(false)}
      >
        Decline
      </button>
      <button
        className="rounded-md bg-neutral-800 px-3 py-1 text-[13px] text-white transition-colors hover:bg-neutral-700"
        onClick={() => respond(true)}
        autoFocus
      >
        Approve
      </button>
    </div>
  );
}

export function useToolApproval(running: boolean) {
  const { pending } = useElicitation();
  return running && !!pending;
}

export function MarkdownPreview({ children }: { children: string }) {
  return (
    <div className="rounded-lg border border-neutral-200/80 bg-white">
      <div className="max-h-64 overflow-y-auto px-3 py-2.5">
        <Streamdown
          className="text-[13px] leading-relaxed text-neutral-700"
          linkSafety={{ enabled: false }}
        >
          {children}
        </Streamdown>
      </div>
    </div>
  );
}
