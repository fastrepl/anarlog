import { createServerFn } from "@tanstack/react-start";
import { setResponseHeader } from "@tanstack/react-start/server";
import { z } from "zod";

import { getSupabaseServerClient } from "@/functions/supabase";

const invitationIdSchema = z.string().uuid();
const invitationActionInputSchema = z
  .object({
    invitationId: invitationIdSchema,
    token: z.string().regex(/^[A-Za-z0-9_-]{43}$/),
  })
  .strict();

const invitationStateSchema = z
  .object({
    status: z.enum(["pending", "accepted", "revoked", "expired"]),
    workspace_id: z.string().uuid(),
    workspace_name: z.string().min(1).max(200),
  })
  .strict();

const acceptedInvitationRowSchema = z
  .object({
    workspace_id: z.string().uuid(),
    membership_id: z.string().uuid(),
  })
  .strict();

export type WorkspaceInvitationState = {
  status: "pending" | "accepted" | "revoked" | "expired";
  workspaceId: string;
  workspaceName: string;
};

function setPrivateInvitationResponseHeaders() {
  setResponseHeader("Cache-Control", "private, no-store");
  setResponseHeader("Referrer-Policy", "no-referrer");
  setResponseHeader("X-Robots-Tag", "noindex, nofollow, noarchive, nosnippet");
}

function unavailableOrError(
  code: string | undefined,
): { status: "unavailable" } | { status: "error" } {
  return code === "22023" || code === "42501"
    ? { status: "unavailable" }
    : { status: "error" };
}

export const inspectMyWorkspaceInvitation = createServerFn({ method: "POST" })
  .inputValidator(invitationActionInputSchema)
  .handler(
    async ({
      data: input,
    }): Promise<
      | { status: "ready"; invitation: WorkspaceInvitationState }
      | { status: "unavailable" }
      | { status: "error" }
    > => {
      setPrivateInvitationResponseHeaders();

      const supabase = getSupabaseServerClient();
      const { data, error } = await supabase.rpc(
        "inspect_my_workspace_invitation",
        {
          p_invitation_id: input.invitationId,
          p_invite_token: input.token,
        },
      );
      if (error) return unavailableOrError(error.code);
      if (!Array.isArray(data) || data.length === 0) {
        return { status: "unavailable" };
      }
      if (data.length !== 1) return { status: "error" };

      try {
        const parsed = invitationStateSchema.parse(data[0]);
        return {
          status: "ready",
          invitation: {
            status: parsed.status,
            workspaceId: parsed.workspace_id,
            workspaceName: parsed.workspace_name,
          },
        };
      } catch {
        return { status: "error" };
      }
    },
  );

export const acceptWorkspaceInvitation = createServerFn({ method: "POST" })
  .inputValidator(invitationActionInputSchema)
  .handler(
    async ({
      data: input,
    }): Promise<
      | { status: "ready"; workspaceId: string }
      | { status: "unavailable" }
      | { status: "error" }
    > => {
      setPrivateInvitationResponseHeaders();

      const supabase = getSupabaseServerClient();
      const { data, error } = await supabase.rpc(
        "accept_workspace_invitation",
        {
          p_invitation_id: input.invitationId,
          p_invite_token: input.token,
        },
      );
      if (error) return unavailableOrError(error.code);
      if (!Array.isArray(data) || data.length === 0) {
        return { status: "unavailable" };
      }
      if (data.length !== 1) return { status: "error" };

      try {
        const accepted = acceptedInvitationRowSchema.parse(data[0]);
        return { status: "ready", workspaceId: accepted.workspace_id };
      } catch {
        return { status: "error" };
      }
    },
  );
