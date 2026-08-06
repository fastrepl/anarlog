import { describe, expect, it } from "vitest";

import {
  buildAccountSessionShareUrl,
  buildPublicSessionShareUrl,
  buildSessionInvitationUrl,
  buildSessionShareLinkUrl,
  createSessionShareLinkPreviewToken,
} from "./urls";

const shareId = "33333333-3333-4333-8333-333333333333";
const invitationId = "55555555-5555-4555-8555-555555555555";
const token = "t".repeat(43);
const previewToken = "a".repeat(64);
const publicSlug = `s_${"a".repeat(32)}`;

describe("session share URLs", () => {
  it("places bearer link tokens only in the fragment", () => {
    const url = new URL(
      buildSessionShareLinkUrl({
        appBaseUrl: "https://anarlog.so",
        shareId,
        linkToken: token,
        previewToken,
      }),
    );

    expect(url.pathname).toBe(`/share/link/${shareId}/`);
    expect(url.searchParams.get("preview")).toBe(previewToken);
    expect(url.searchParams.has("token")).toBe(false);
    expect(url.hash).toBe(`#token=${token}`);
  });

  it("derives a metadata-only preview token from the bearer token", async () => {
    await expect(createSessionShareLinkPreviewToken(token)).resolves.toBe(
      "80383f974f22964fd6b7ae851b6ccc9180ed4e6fcb2e415bafcab6d822139238",
    );
  });

  it("places invitation tokens only in the fragment", () => {
    const url = new URL(
      buildSessionInvitationUrl({
        appBaseUrl: "https://anarlog.so",
        invitationId,
        inviteToken: token,
      }),
    );

    expect(url.pathname).toBe(`/share/invite/${invitationId}/`);
    expect(url.search).toBe("");
    expect(url.hash).toBe(`#token=${token}`);
  });

  it("builds token-free account and public URLs", () => {
    expect(
      buildAccountSessionShareUrl({
        appBaseUrl: "https://anarlog.so",
        shareId,
      }),
    ).toBe(`https://anarlog.so/share/${shareId}/`);
    expect(
      buildPublicSessionShareUrl({
        appBaseUrl: "https://anarlog.so",
        publicSlug,
      }),
    ).toBe(`https://anarlog.so/share/public/${publicSlug}/`);
  });

  it("targets non-stable builds without changing stable canonical URLs", () => {
    const linkUrl = new URL(
      buildSessionShareLinkUrl({
        appBaseUrl: "https://anarlog.so",
        shareId,
        linkToken: token,
        previewToken,
        desktopScheme: "anarlog-staging",
      }),
    );
    expect(linkUrl.searchParams.get("scheme")).toBe("anarlog-staging");
    expect(linkUrl.searchParams.get("preview")).toBe(previewToken);
    expect(linkUrl.hash).toBe(`#token=${token}`);

    const publicUrl = new URL(
      buildPublicSessionShareUrl({
        appBaseUrl: "https://anarlog.so",
        publicSlug,
        desktopScheme: "anarlog-dev",
      }),
    );
    expect(publicUrl.searchParams.get("scheme")).toBe("anarlog-dev");

    const stableUrl = new URL(
      buildAccountSessionShareUrl({
        appBaseUrl: "https://anarlog.so",
        shareId,
        desktopScheme: "anarlog",
      }),
    );
    expect(stableUrl.search).toBe("");
  });

  it("rejects tokens or base URLs that could escape the canonical shape", () => {
    expect(() =>
      buildSessionShareLinkUrl({
        appBaseUrl: "javascript:alert(1)",
        shareId,
        linkToken: token,
        previewToken,
      }),
    ).toThrow("Share URL is unavailable");
    expect(() =>
      buildSessionShareLinkUrl({
        appBaseUrl: "https://anarlog.so?token=old",
        shareId,
        linkToken: token,
        previewToken,
      }),
    ).toThrow("Share URL is unavailable");
    expect(() =>
      buildSessionShareLinkUrl({
        appBaseUrl: "https://anarlog.so",
        shareId,
        linkToken: "bad?token",
        previewToken,
      }),
    ).toThrow("Share URL is unavailable");
    expect(() =>
      buildSessionShareLinkUrl({
        appBaseUrl: "https://anarlog.so",
        shareId,
        linkToken: token,
        previewToken: "bad-preview",
      }),
    ).toThrow("Share URL is unavailable");
  });
});
