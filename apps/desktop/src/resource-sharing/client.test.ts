import { describe, expect, it, vi } from "vitest";

import {
  isTeamSharingUpsellError,
  moveSharedResource,
  requireResourceSharingContext,
  ResourceSharingError,
} from "./client";

describe("resource sharing client", () => {
  it("recognizes the server-enforced multi-resource Team upgrade", () => {
    expect(
      isTeamSharingUpsellError(
        new ResourceSharingError(
          "multi-resource guest requires Team membership",
        ),
      ),
    ).toBe(true);
    expect(
      isTeamSharingUpsellError(new ResourceSharingError("not permitted")),
    ).toBe(false);
  });

  it("requires a permanent signed-in session", () => {
    expect(() => requireResourceSharingContext({})).toThrow(
      ResourceSharingError,
    );
    expect(() =>
      requireResourceSharingContext({
        supabase: {} as never,
        session: { user: { is_anonymous: true } } as never,
      }),
    ).toThrow(ResourceSharingError);
  });

  it("moves a shared resource without replacing its share", async () => {
    const setHeader = vi.fn(() => Promise.resolve({ data: [], error: null }));
    const rpc = vi.fn(() => ({ setHeader }));

    await moveSharedResource(
      {
        supabase: { rpc } as never,
        session: { access_token: "access-token" } as never,
      },
      {
        shareId: "share-1",
        sourceId: "Renamed Customers",
        title: "Renamed Customers",
        payload: { version: 1, path: "Renamed Customers", notes: [] },
      },
    );

    expect(rpc).toHaveBeenCalledWith("move_shared_resource", {
      p_share_id: "share-1",
      p_source_id: "Renamed Customers",
      p_title: "Renamed Customers",
      p_payload: { version: 1, path: "Renamed Customers", notes: [] },
    });
    expect(setHeader).toHaveBeenCalledWith(
      "Authorization",
      "Bearer access-token",
    );
  });
});
