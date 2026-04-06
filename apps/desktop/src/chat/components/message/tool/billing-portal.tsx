import { ExternalLinkIcon } from "lucide-react";

import { defineTool } from "./define-tool";
import { ToolCardFooterSuccess } from "./shared";

import { parseCreateBillingPortalSessionOutput } from "~/chat/mcp/support-mcp-tools";

export const ToolBillingPortal = defineTool({
  icon: <ExternalLinkIcon />,
  parseFn: parseCreateBillingPortalSessionOutput,
  label: ({ running, failed, parsed }) => {
    if (running) return "Creating billing portal...";
    if (failed) return "Billing portal failed";
    if (parsed) return "Billing portal ready";
    return "Billing portal";
  },
  renderSuccess: (parsed) => (
    <ToolCardFooterSuccess href={parsed.url} label="Open billing portal" />
  ),
});
