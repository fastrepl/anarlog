import { describe, expect, it } from "vitest";

import { getProviderProfileImageUrl, getProviderProfileName } from "./profile";

describe("provider profile", () => {
  it("prefers the provider avatar URL from user metadata", () => {
    expect(
      getProviderProfileImageUrl({
        email: "ada@example.com",
        identities: [
          {
            created_at: "2026-01-01T00:00:00.000Z",
            id: "google-id",
            identity_data: { picture: "https://google.example/identity.png" },
            identity_id: "identity-id",
            provider: "google",
            user_id: "user-id",
          },
        ],
        user_metadata: {
          avatar_url: "https://google.example/user.png",
          picture: "https://google.example/picture.png",
        },
      }),
    ).toBe("https://google.example/user.png");
  });

  it("falls back to linked identity metadata", () => {
    expect(
      getProviderProfileImageUrl({
        email: "ada@example.com",
        identities: [
          {
            created_at: "2026-01-01T00:00:00.000Z",
            id: "github-id",
            identity_data: { avatar_url: "https://github.example/ada.png" },
            identity_id: "identity-id",
            provider: "github",
            user_id: "user-id",
          },
        ],
        user_metadata: {},
      }),
    ).toBe("https://github.example/ada.png");
  });

  it("reads Google's picture metadata", () => {
    expect(
      getProviderProfileImageUrl({
        email: "ada@example.com",
        identities: [],
        user_metadata: { picture: "https://google.example/ada.png" },
      }),
    ).toBe("https://google.example/ada.png");
  });

  it("rejects unsafe and malformed image URLs", () => {
    expect(
      getProviderProfileImageUrl({
        email: "ada@example.com",
        identities: [],
        user_metadata: { avatar_url: "javascript:alert(1)" },
      }),
    ).toBeNull();
    expect(
      getProviderProfileImageUrl({
        email: "ada@example.com",
        identities: [],
        user_metadata: { picture: "not a URL" },
      }),
    ).toBeNull();
  });

  it("uses provider names before the account email", () => {
    expect(
      getProviderProfileName({
        email: "ada@example.com",
        identities: [],
        user_metadata: { full_name: "Ada Lovelace", name: "Ada" },
      }),
    ).toBe("Ada Lovelace");
    expect(
      getProviderProfileName({
        email: "ada@example.com",
        identities: [],
        user_metadata: {},
      }),
    ).toBe("ada@example.com");
  });
});
