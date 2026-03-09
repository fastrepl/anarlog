import { ExternalLinkIcon } from "lucide-react";

import {
  ToolCard,
  ToolCardFooters,
  ToolCardFooterSuccess,
  ToolCardHeader,
  useMcpOutput,
  useToolState,
} from "./shared";

import type { ToolRenderer } from "~/chat/components/message/types";
import { parseCreateBillingPortalSessionOutput } from "~/chat/mcp/support-mcp-tools";

type Renderer = ToolRenderer<"tool-create_billing_portal_session">;

function headerLabel(
  running: boolean,
  failed: boolean,
  parsed: ReturnType<typeof parseCreateBillingPortalSessionOutput>,
): string {
  if (running) return "Creating billing portal...";
  if (failed) return "Billing portal failed";
  if (parsed) return "Billing portal ready";
  return "Billing portal";
}

export const ToolBillingPortal: Renderer = ({ part }) => {
  const { running, failed, done } = useToolState(part);
  const { parsed, rawText } = useMcpOutput(
    done,
    part.output,
    parseCreateBillingPortalSessionOutput,
  );

  return (
    <ToolCard failed={failed}>
      <ToolCardHeader
        icon={<ExternalLinkIcon />}
        running={running}
        failed={failed}
        done={!!parsed}
        label={headerLabel(running, failed, parsed)}
      />

      <ToolCardFooters
        failed={failed}
        errorText={part.errorText}
        rawText={rawText}
      >
        {parsed ? (
          <ToolCardFooterSuccess
            href={parsed.url}
            label="Open billing portal"
          />
        ) : null}
      </ToolCardFooters>
    </ToolCard>
  );
};
