import { describe, expect, test, vi } from "vitest";

import type { CrmContact } from "./connection";

const connection = vi.hoisted(() => ({
  lookupCrmContacts: vi.fn(),
}));
const queries = vi.hoisted(() => ({
  applyContactEnhancement: vi.fn(async () => {}),
}));

vi.mock("./connection", async () => {
  const actual = await vi.importActual("./connection");
  return { ...actual, lookupCrmContacts: connection.lookupCrmContacts };
});
vi.mock("~/contacts/queries", () => queries);

import {
  enrichHumanFromCrm,
  linkedinUsernameFromUrl,
  pickBestCrmContact,
  planCrmEnrichment,
} from "./enrichment";

const human = {
  id: "h1",
  name: "Ada Lovelace",
  email: "ada@example.com",
  jobTitle: "",
  phone: "",
  linkedinUsername: "",
  organizationId: "",
};

const contact = (overrides: Partial<CrmContact> = {}): CrmContact => ({
  id: "c1",
  name: "Ada Lovelace",
  email: "ada@example.com",
  companyName: "Analytical Engines",
  jobTitle: "Engineer",
  phone: "+44 20 7946 0000",
  linkedinUrl: "https://www.linkedin.com/in/ada-lovelace/",
  url: "https://crm.example.com/c1",
  ...overrides,
});

describe("planCrmEnrichment", () => {
  test("fills only empty fields", () => {
    expect(planCrmEnrichment(human, contact())).toEqual({
      companyName: "Analytical Engines",
      jobTitle: "Engineer",
      phone: "+44 20 7946 0000",
      linkedinUsername: "ada-lovelace",
    });
  });

  test("never overwrites local values", () => {
    const filled = {
      ...human,
      jobTitle: "CTO",
      phone: "123",
      linkedinUsername: "ada",
      organizationId: "org1",
    };
    expect(planCrmEnrichment(filled, contact())).toEqual({});
  });

  test("ignores blank CRM values", () => {
    expect(
      planCrmEnrichment(
        human,
        contact({ jobTitle: "  ", phone: null, linkedinUrl: null }),
      ),
    ).toEqual({ companyName: "Analytical Engines" });
  });
});

describe("linkedinUsernameFromUrl", () => {
  test("extracts the profile slug", () => {
    expect(linkedinUsernameFromUrl("https://linkedin.com/in/ada?x=1")).toBe(
      "ada",
    );
    expect(linkedinUsernameFromUrl("ada")).toBe("ada");
    expect(linkedinUsernameFromUrl("https://example.com/ada")).toBeUndefined();
  });
});

describe("pickBestCrmContact", () => {
  test("prefers the record that adds the most fields", () => {
    const sparse = contact({ id: "sparse", jobTitle: null, phone: null });
    const best = pickBestCrmContact(human, [sparse, contact()]);
    expect(best?.contact.id).toBe("c1");
  });

  test("returns null without candidates", () => {
    expect(pickBestCrmContact(human, [])).toBeNull();
  });
});

describe("enrichHumanFromCrm", () => {
  const providers = [
    { id: "hubspot", name: "HubSpot", nangoIntegrationId: "hubspot" },
    { id: "attio", name: "Attio", nangoIntegrationId: "attio" },
  ];
  const headers = { Authorization: "Bearer test" };
  const connections = [
    {
      integration_id: "hubspot",
      connection_id: "conn-hubspot",
      status: "connected",
    },
    {
      integration_id: "attio",
      connection_id: "conn-attio",
      status: "connected",
    },
  ];
  const enrich = (
    overrides: Partial<Parameters<typeof enrichHumanFromCrm>[0]> = {},
  ) =>
    enrichHumanFromCrm({
      human,
      ownerUserId: "u1",
      providers,
      connections,
      headers,
      ...overrides,
    });

  test("reports when no provider is connected", async () => {
    await expect(enrich({ connections: [] })).resolves.toEqual({
      status: "not_connected",
    });
    await expect(enrich({ connections: undefined })).resolves.toEqual({
      status: "not_connected",
    });
    expect(connection.lookupCrmContacts).not.toHaveBeenCalled();
  });

  test("skips providers whose connection needs reconnect", async () => {
    connection.lookupCrmContacts.mockReset().mockResolvedValue([contact()]);
    const result = await enrich({
      connections: connections.map((item) => ({
        ...item,
        status: "reconnect_required",
      })),
    });
    expect(result.status).toBe("not_connected");
    expect(connection.lookupCrmContacts).not.toHaveBeenCalled();
  });

  test("applies the first match and stops", async () => {
    connection.lookupCrmContacts.mockReset().mockResolvedValueOnce([contact()]);
    const result = await enrich();
    expect(result.status).toBe("matched");
    expect(connection.lookupCrmContacts).toHaveBeenCalledTimes(1);
    expect(connection.lookupCrmContacts).toHaveBeenCalledWith(
      providers[0],
      "conn-hubspot",
      { email: "ada@example.com", name: "Ada Lovelace" },
      headers,
    );
    expect(queries.applyContactEnhancement).toHaveBeenCalledWith({
      humanId: "h1",
      ownerUserId: "u1",
      changes: {
        companyName: "Analytical Engines",
        jobTitle: "Engineer",
        phone: "+44 20 7946 0000",
        linkedinUsername: "ada-lovelace",
      },
    });
  });

  test("falls through to the next provider and reports no match", async () => {
    connection.lookupCrmContacts.mockReset().mockResolvedValue([]);
    queries.applyContactEnhancement.mockClear();
    await expect(enrich()).resolves.toEqual({
      status: "no_match",
      providers: ["HubSpot", "Attio"],
    });
    expect(connection.lookupCrmContacts).toHaveBeenCalledTimes(2);
    expect(queries.applyContactEnhancement).not.toHaveBeenCalled();
  });

  test("surfaces a provider error only when nothing matched", async () => {
    connection.lookupCrmContacts
      .mockReset()
      .mockRejectedValueOnce(new Error("HubSpot down"))
      .mockResolvedValueOnce([]);
    await expect(enrich()).rejects.toThrow("HubSpot down");
  });
});
