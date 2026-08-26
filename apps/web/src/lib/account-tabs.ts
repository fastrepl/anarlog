export const ACCOUNT_SECTIONS = [
  { id: "profile", label: "Profile" },
  { id: "plan", label: "Your plan" },
  { id: "referrals", label: "Refer friends" },
  { id: "integrations", label: "Integrations" },
  { id: "devices", label: "Synced devices" },
  { id: "shares", label: "Shared notes" },
  { id: "api-keys", label: "Cloud API keys" },
  { id: "session", label: "Session controls" },
  { id: "danger", label: "Danger area" },
] as const;

export type AccountSectionId = (typeof ACCOUNT_SECTIONS)[number]["id"];

export const ACCOUNT_TABS = [
  {
    id: "account",
    label: "Account",
    sectionIds: ["profile", "plan", "referrals", "session", "danger"],
  },
  {
    id: "connections",
    label: "Connections",
    sectionIds: ["integrations", "devices", "shares"],
  },
  {
    id: "developer",
    label: "Developer",
    sectionIds: ["api-keys"],
  },
] as const;

export type AccountTabId = (typeof ACCOUNT_TABS)[number]["id"];

export const DEFAULT_ACCOUNT_TAB: AccountTabId = "account";

const SECTION_TAB: Record<AccountSectionId, AccountTabId> = {
  profile: "account",
  plan: "account",
  referrals: "account",
  session: "account",
  danger: "account",
  integrations: "connections",
  devices: "connections",
  shares: "connections",
  "api-keys": "developer",
};

export function isAccountTabId(value: string): value is AccountTabId {
  return ACCOUNT_TABS.some((tab) => tab.id === value);
}

export function isAccountSectionId(value: string): value is AccountSectionId {
  return ACCOUNT_SECTIONS.some((section) => section.id === value);
}

export function accountTabForSection(
  sectionId: string,
): AccountTabId | undefined {
  if (!isAccountSectionId(sectionId)) {
    return undefined;
  }
  return SECTION_TAB[sectionId];
}

export function resolveAccountTab(input: {
  tab?: string | null;
  hash?: string | null;
}): AccountTabId {
  const hash = input.hash?.replace(/^#/, "").trim();
  if (hash) {
    const fromSection = accountTabForSection(hash);
    if (fromSection) {
      return fromSection;
    }
    if (isAccountTabId(hash)) {
      return hash;
    }
  }

  if (input.tab && isAccountTabId(input.tab)) {
    return input.tab;
  }

  return DEFAULT_ACCOUNT_TAB;
}

export function sectionsForAccountTab(tabId: AccountTabId) {
  const tab = ACCOUNT_TABS.find((item) => item.id === tabId);
  if (!tab) {
    return [];
  }

  return tab.sectionIds
    .map((sectionId) =>
      ACCOUNT_SECTIONS.find((section) => section.id === sectionId),
    )
    .filter((section) => section != null);
}
