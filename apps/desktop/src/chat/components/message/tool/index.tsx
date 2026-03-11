import { ToolAddComment } from "./add-comment";
import { ToolBillingPortal } from "./billing-portal";
import { ToolCreateIssue } from "./create-issue";
import { ToolEditSummary } from "./edit-summary";
import { ToolGeneric } from "./generic";
import { ToolListSubscriptions } from "./list-subscriptions";
import { ToolSearchSessions } from "./search";
import { ToolSearchIssues } from "./search-issues";

import type { Part } from "~/chat/components/message/types";

const toolRegistry: Record<string, (props: { part: Part }) => React.ReactNode> =
  {
    "tool-search_sessions": ToolSearchSessions as (props: {
      part: Part;
    }) => React.ReactNode,
    "tool-create_issue": ToolCreateIssue as (props: {
      part: Part;
    }) => React.ReactNode,
    "tool-add_comment": ToolAddComment as (props: {
      part: Part;
    }) => React.ReactNode,
    "tool-search_issues": ToolSearchIssues as (props: {
      part: Part;
    }) => React.ReactNode,
    "tool-list_subscriptions": ToolListSubscriptions as (props: {
      part: Part;
    }) => React.ReactNode,
    "tool-create_billing_portal_session": ToolBillingPortal as (props: {
      part: Part;
    }) => React.ReactNode,
    "tool-edit_summary": ToolEditSummary as (props: {
      part: Part;
    }) => React.ReactNode,
  };

export function Tool({ part }: { part: Part }) {
  const Renderer = toolRegistry[part.type];
  if (Renderer) {
    return <Renderer part={part} />;
  }
  return <ToolGeneric part={part as Record<string, unknown>} />;
}
