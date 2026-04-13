import { ToolAddComment } from "./add-comment";
import { ToolBillingPortal } from "./billing-portal";
import { ToolCreateIssue } from "./create-issue";
import { ToolEditSummary } from "./edit-summary";
import { ToolGeneric } from "./generic";
import { ToolListSubscriptions } from "./list-subscriptions";
import { ToolSearchSessions } from "./search";
import { ToolSearchIssues } from "./search-issues";
import { ToolUpdatePromptTemplate } from "./update-prompt-template";

import type { Part } from "~/chat/components/message/types";

type ToolComponent = (props: { part: Part }) => React.ReactNode;

const toolRegistry: Record<string, ToolComponent> = {
  "tool-search_sessions": ToolSearchSessions as ToolComponent,
  "tool-create_issue": ToolCreateIssue as ToolComponent,
  "tool-add_comment": ToolAddComment as ToolComponent,
  "tool-search_issues": ToolSearchIssues as ToolComponent,
  "tool-list_subscriptions": ToolListSubscriptions as ToolComponent,
  "tool-create_billing_portal_session": ToolBillingPortal as ToolComponent,
  "tool-edit_summary": ToolEditSummary as ToolComponent,
  "tool-update_prompt_template": ToolUpdatePromptTemplate as ToolComponent,
};

export function Tool({ part }: { part: Part }) {
  const Renderer = toolRegistry[part.type];
  if (Renderer) {
    return <Renderer part={part} />;
  }
  return <ToolGeneric part={part as Record<string, unknown>} />;
}
