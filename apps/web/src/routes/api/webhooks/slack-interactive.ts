import { createFileRoute } from "@tanstack/react-router";
import * as crypto from "crypto";

import { getGitHubCredentials } from "@/functions/github-content";

const GITHUB_REPO = "fastrepl/char";

function verifySlackSignature(
  signingSecret: string,
  timestamp: string,
  body: string,
  signature: string,
): boolean {
  const baseString = `v0:${timestamp}:${body}`;
  const hmac = crypto.createHmac("sha256", signingSecret);
  hmac.update(baseString);
  const computedSignature = `v0=${hmac.digest("hex")}`;

  const a = Buffer.from(computedSignature);
  const b = Buffer.from(signature);
  if (a.length !== b.length) return false;
  return crypto.timingSafeEqual(a, b);
}

async function mergePullRequest(
  prNumber: number,
  token: string,
): Promise<{ success: boolean; error?: string }> {
  try {
    const response = await fetch(
      `https://api.github.com/repos/${GITHUB_REPO}/pulls/${prNumber}/merge`,
      {
        method: "PUT",
        headers: {
          Authorization: `Bearer ${token}`,
          Accept: "application/vnd.github.v3+json",
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          merge_method: "squash",
        }),
      },
    );

    if (!response.ok) {
      const error = await response.json();
      return {
        success: false,
        error: error.message || `GitHub API error: ${response.status}`,
      };
    }

    return { success: true };
  } catch (error) {
    return {
      success: false,
      error: `Failed to merge PR: ${(error as Error).message}`,
    };
  }
}

async function deleteBranch(
  branchName: string,
  token: string,
): Promise<{ success: boolean; error?: string }> {
  try {
    const response = await fetch(
      `https://api.github.com/repos/${GITHUB_REPO}/git/refs/heads/${branchName}`,
      {
        method: "DELETE",
        headers: {
          Authorization: `Bearer ${token}`,
          Accept: "application/vnd.github.v3+json",
        },
      },
    );

    if (!response.ok && response.status !== 422) {
      const error = await response.json();
      return {
        success: false,
        error: error.message || `GitHub API error: ${response.status}`,
      };
    }

    return { success: true };
  } catch (error) {
    return {
      success: false,
      error: `Failed to delete branch: ${(error as Error).message}`,
    };
  }
}

async function getPullRequestBranch(
  prNumber: number,
  token: string,
): Promise<{ success: boolean; branchName?: string; error?: string }> {
  try {
    const response = await fetch(
      `https://api.github.com/repos/${GITHUB_REPO}/pulls/${prNumber}`,
      {
        headers: {
          Authorization: `Bearer ${token}`,
          Accept: "application/vnd.github.v3+json",
        },
      },
    );

    if (!response.ok) {
      const error = await response.json();
      return {
        success: false,
        error: error.message || `GitHub API error: ${response.status}`,
      };
    }

    const data = await response.json();
    return { success: true, branchName: data.head.ref };
  } catch (error) {
    return {
      success: false,
      error: `Failed to get PR info: ${(error as Error).message}`,
    };
  }
}

export const Route = createFileRoute("/api/webhooks/slack-interactive")({
  server: {
    handlers: {
      POST: async ({ request }) => {
        const signingSecret = process.env.SLACK_SIGNING_SECRET;
        if (!signingSecret) {
          return new Response(
            JSON.stringify({ error: "Slack signing secret not configured" }),
            { status: 500, headers: { "Content-Type": "application/json" } },
          );
        }

        const timestamp = request.headers.get("x-slack-request-timestamp");
        const signature = request.headers.get("x-slack-signature");

        if (!timestamp || !signature) {
          return new Response(
            JSON.stringify({ error: "Missing Slack signature headers" }),
            { status: 400, headers: { "Content-Type": "application/json" } },
          );
        }

        const now = Math.floor(Date.now() / 1000);
        if (Math.abs(now - parseInt(timestamp)) > 60 * 5) {
          return new Response(JSON.stringify({ error: "Request too old" }), {
            status: 400,
            headers: { "Content-Type": "application/json" },
          });
        }

        const rawBody = await request.text();

        if (
          !verifySlackSignature(signingSecret, timestamp, rawBody, signature)
        ) {
          return new Response(JSON.stringify({ error: "Invalid signature" }), {
            status: 401,
            headers: { "Content-Type": "application/json" },
          });
        }

        const params = new URLSearchParams(rawBody);
        const payloadStr = params.get("payload");
        if (!payloadStr) {
          return new Response(JSON.stringify({ error: "Missing payload" }), {
            status: 400,
            headers: { "Content-Type": "application/json" },
          });
        }

        let payload: {
          type: string;
          actions?: Array<{
            action_id: string;
            value: string;
          }>;
          response_url?: string;
        };

        try {
          payload = JSON.parse(payloadStr);
        } catch {
          return new Response(JSON.stringify({ error: "Invalid payload" }), {
            status: 400,
            headers: { "Content-Type": "application/json" },
          });
        }

        if (payload.type !== "block_actions") {
          return new Response(JSON.stringify({ ok: true }), {
            status: 200,
            headers: { "Content-Type": "application/json" },
          });
        }

        const action = payload.actions?.[0];
        if (!action || action.action_id !== "merge_pr") {
          return new Response(JSON.stringify({ ok: true }), {
            status: 200,
            headers: { "Content-Type": "application/json" },
          });
        }

        const prNumber = parseInt(action.value);
        if (isNaN(prNumber)) {
          return new Response(JSON.stringify({ error: "Invalid PR number" }), {
            status: 400,
            headers: { "Content-Type": "application/json" },
          });
        }

        const credentials = await getGitHubCredentials();
        if (!credentials?.token) {
          return new Response(
            JSON.stringify({ error: "GitHub token not configured" }),
            { status: 500, headers: { "Content-Type": "application/json" } },
          );
        }

        const branchResult = await getPullRequestBranch(
          prNumber,
          credentials.token,
        );
        if (!branchResult.success) {
          if (payload.response_url) {
            await fetch(payload.response_url, {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({
                text: `Failed to get PR info: ${branchResult.error}`,
                response_type: "ephemeral",
              }),
            });
          }
          return new Response(JSON.stringify({ ok: true }), {
            status: 200,
            headers: { "Content-Type": "application/json" },
          });
        }

        const mergeResult = await mergePullRequest(prNumber, credentials.token);
        if (!mergeResult.success) {
          if (payload.response_url) {
            await fetch(payload.response_url, {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({
                text: `Failed to merge PR #${prNumber}: ${mergeResult.error}`,
                response_type: "ephemeral",
              }),
            });
          }
          return new Response(JSON.stringify({ ok: true }), {
            status: 200,
            headers: { "Content-Type": "application/json" },
          });
        }

        if (branchResult.branchName) {
          await deleteBranch(branchResult.branchName, credentials.token);
        }

        if (payload.response_url) {
          await fetch(payload.response_url, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
              text: `PR #${prNumber} merged successfully and branch deleted.`,
              response_type: "in_channel",
            }),
          });
        }

        return new Response(JSON.stringify({ ok: true }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        });
      },
    },
  },
});
