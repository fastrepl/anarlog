import { renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import {
  TinyBaseTestWrapper,
  useHuman,
  useOrganization,
  useSession,
  useSetSessionRawMd,
  useSetSessionTitle,
  useTemplate,
} from ".";

describe("TinyBase hooks", () => {
  describe("useSession", () => {
    it("returns an object with session fields", () => {
      const { result } = renderHook(() => useSession("test-session"), {
        wrapper: ({ children }) => (
          <TinyBaseTestWrapper>{children}</TinyBaseTestWrapper>
        ),
      });

      expect(result.current).toHaveProperty("title");
      expect(result.current).toHaveProperty("rawMd");
      expect(result.current).toHaveProperty("createdAt");
      expect(result.current).toHaveProperty("event");
      expect(result.current).toHaveProperty("folderId");
    });

    it("returns undefined for non-existent session", () => {
      const { result } = renderHook(() => useSession("non-existent"), {
        wrapper: ({ children }) => (
          <TinyBaseTestWrapper>{children}</TinyBaseTestWrapper>
        ),
      });

      expect(result.current.title).toBeUndefined();
      expect(result.current.rawMd).toBeUndefined();
    });
  });

  describe("useSetSessionTitle", () => {
    it("returns a function", () => {
      const { result } = renderHook(() => useSetSessionTitle(), {
        wrapper: ({ children }) => (
          <TinyBaseTestWrapper>{children}</TinyBaseTestWrapper>
        ),
      });

      expect(typeof result.current).toBe("function");
    });
  });

  describe("useSetSessionRawMd", () => {
    it("returns a function", () => {
      const { result } = renderHook(() => useSetSessionRawMd(), {
        wrapper: ({ children }) => (
          <TinyBaseTestWrapper>{children}</TinyBaseTestWrapper>
        ),
      });

      expect(typeof result.current).toBe("function");
    });
  });

  describe("useHuman", () => {
    it("returns an object with human fields", () => {
      const { result } = renderHook(() => useHuman("test-human"), {
        wrapper: ({ children }) => (
          <TinyBaseTestWrapper>{children}</TinyBaseTestWrapper>
        ),
      });

      expect(result.current).toHaveProperty("name");
      expect(result.current).toHaveProperty("email");
      expect(result.current).toHaveProperty("orgId");
      expect(result.current).toHaveProperty("jobTitle");
      expect(result.current).toHaveProperty("linkedinUsername");
    });
  });

  describe("useOrganization", () => {
    it("returns an object with organization fields", () => {
      const { result } = renderHook(() => useOrganization("test-org"), {
        wrapper: ({ children }) => (
          <TinyBaseTestWrapper>{children}</TinyBaseTestWrapper>
        ),
      });

      expect(result.current).toHaveProperty("name");
    });
  });

  describe("useTemplate", () => {
    it("returns an object with template fields", () => {
      const { result } = renderHook(() => useTemplate("test-template"), {
        wrapper: ({ children }) => (
          <TinyBaseTestWrapper>{children}</TinyBaseTestWrapper>
        ),
      });

      expect(result.current).toHaveProperty("title");
      expect(result.current).toHaveProperty("description");
      expect(result.current).toHaveProperty("sections");
    });
  });

  describe("TinyBaseTestWrapper", () => {
    it("provides store context for hooks", () => {
      const { result } = renderHook(
        () => ({
          session: useSession("session-1"),
          human: useHuman("human-1"),
        }),
        {
          wrapper: ({ children }) => (
            <TinyBaseTestWrapper>{children}</TinyBaseTestWrapper>
          ),
        },
      );

      expect(result.current.session).toHaveProperty("title");
      expect(result.current.human).toHaveProperty("name");
    });
  });
});
