import type { McpTextContentOutput } from "./mcp-output-parser";
import { parseMcpToolOutput } from "./mcp-output-parser";

export type AddCommentParams = {
  issue_number: number;
  body: string;
};

type AddCommentOutput = {
  success: boolean;
  comment_url: string;
};

export type CreateBillingPortalSessionParams = {
  return_url?: string | null;
};

type CreateBillingPortalSessionOutput = {
  url: string;
};

export type CreateIssueParams = {
  title: string;
  body: string;
  issue_type?: string | null;
  labels?: string[] | null;
};

type CreateIssueOutput = {
  success: boolean;
  issue_url: string;
  issue_number: number;
  labels?: string[] | null;
};

export type ListSubscriptionsParams = {
  status?: string | null;
};

export type SearchIssueItem = {
  number: number;
  title: string;
  state: string;
  url: string;
  created_at: string;
  labels: string[];
};

type SearchIssuesOutput = {
  total_results: number;
  issues: SearchIssueItem[];
};

export type SearchIssuesParams = {
  query: string;
  state?: string | null;
  limit?: number | null;
};

type SubscriptionItem = {
  id: string;
  status: string;
  start_date: number | null;
  cancel_at_period_end: boolean;
  cancel_at: number | null;
  canceled_at: number | null;
  trial_start: number | null;
  trial_end: number | null;
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export type SupportMcpTools = {
  create_issue: { input: CreateIssueParams; output: McpTextContentOutput };
  add_comment: { input: AddCommentParams; output: McpTextContentOutput };
  search_issues: { input: SearchIssuesParams; output: McpTextContentOutput };
  list_subscriptions: {
    input: ListSubscriptionsParams;
    output: McpTextContentOutput;
  };
  create_billing_portal_session: {
    input: CreateBillingPortalSessionParams;
    output: McpTextContentOutput;
  };
};

function isCreateIssueOutput(value: unknown): value is CreateIssueOutput {
  return (
    isRecord(value) &&
    typeof value.success === "boolean" &&
    typeof value.issue_url === "string" &&
    typeof value.issue_number === "number"
  );
}

function isAddCommentOutput(value: unknown): value is AddCommentOutput {
  return (
    isRecord(value) &&
    typeof value.success === "boolean" &&
    typeof value.comment_url === "string"
  );
}

function isNullableNumber(value: unknown): value is number | null {
  return value === null || typeof value === "number";
}

function isSubscriptionItem(value: unknown): value is SubscriptionItem {
  return (
    isRecord(value) &&
    typeof value.id === "string" &&
    typeof value.status === "string" &&
    isNullableNumber(value.start_date) &&
    typeof value.cancel_at_period_end === "boolean" &&
    isNullableNumber(value.cancel_at) &&
    isNullableNumber(value.canceled_at) &&
    isNullableNumber(value.trial_start) &&
    isNullableNumber(value.trial_end)
  );
}

function isSearchIssueItem(value: unknown): value is SearchIssueItem {
  return (
    isRecord(value) &&
    typeof value.number === "number" &&
    typeof value.title === "string" &&
    typeof value.state === "string" &&
    typeof value.url === "string" &&
    typeof value.created_at === "string" &&
    Array.isArray(value.labels) &&
    value.labels.every((label) => typeof label === "string")
  );
}

function isSearchIssuesOutput(value: unknown): value is SearchIssuesOutput {
  return (
    isRecord(value) &&
    typeof value.total_results === "number" &&
    Array.isArray(value.issues) &&
    value.issues.every(isSearchIssueItem)
  );
}

function isSubscriptionList(value: unknown): value is SubscriptionItem[] {
  return Array.isArray(value) && value.every(isSubscriptionItem);
}

function isBillingPortalOutput(
  value: unknown,
): value is CreateBillingPortalSessionOutput {
  return isRecord(value) && typeof value.url === "string";
}

export function parseCreateIssueOutput(
  output: unknown,
): CreateIssueOutput | null {
  return parseMcpToolOutput(output, isCreateIssueOutput);
}

export function parseAddCommentOutput(
  output: unknown,
): AddCommentOutput | null {
  return parseMcpToolOutput(output, isAddCommentOutput);
}

export function parseSearchIssuesOutput(
  output: unknown,
): SearchIssuesOutput | null {
  return parseMcpToolOutput(output, isSearchIssuesOutput);
}

export function parseListSubscriptionsOutput(
  output: unknown,
): SubscriptionItem[] | null {
  return parseMcpToolOutput(output, isSubscriptionList);
}

export function parseCreateBillingPortalSessionOutput(
  output: unknown,
): CreateBillingPortalSessionOutput | null {
  return parseMcpToolOutput(output, isBillingPortalOutput);
}
