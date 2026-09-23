import { describe, expect, test } from "vitest";

import { deriveContactIdentity, inferCompanyNameFromEmail } from "./identity";

describe("inferCompanyNameFromEmail", () => {
  test.each([
    ["simon@ionprotocol.io", "Ionprotocol"],
    ["a@mail.acme.com", "Acme"],
    ["a@acme.co.uk", "Acme"],
    ["a@university.edu.au", "University"],
    ["a@agency.gov.uk", "Agency"],
    ["a@kakao.co.kr", "Kakao"],
    ["a@gmail.com", undefined],
    ["a@localhost", undefined],
  ])("%s -> %s", (email, expected) => {
    expect(inferCompanyNameFromEmail(email)).toBe(expected);
  });
});

describe("deriveContactIdentity", () => {
  test("keeps a provider name that looks like a person", () => {
    expect(
      deriveContactIdentity({ name: "Jane Doe", email: "jd@acme.com" }),
    ).toEqual({
      name: "Jane Doe",
      nameSource: "provider",
      companyName: "Acme",
    });
  });

  test("derives from the email when the provider name is an email", () => {
    expect(
      deriveContactIdentity({
        name: "jane.doe@gmail.com",
        email: "jane.doe@gmail.com",
      }),
    ).toEqual({ name: "Jane Doe", nameSource: "email" });
  });
});
