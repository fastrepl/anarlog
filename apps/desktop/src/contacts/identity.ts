const PERSONAL_EMAIL_DOMAINS = new Set([
  "gmail.com",
  "googlemail.com",
  "yahoo.com",
  "outlook.com",
  "hotmail.com",
  "live.com",
  "msn.com",
  "icloud.com",
  "me.com",
  "mac.com",
  "aol.com",
  "proton.me",
  "protonmail.com",
  "pm.me",
  "hey.com",
  "fastmail.com",
]);

export type DerivedContactIdentity = {
  name: string;
  companyName?: string;
};

export function deriveContactIdentity({
  name,
  email,
}: {
  name?: string | null;
  email: string;
}): DerivedContactIdentity {
  const trimmedName = name?.trim() ?? "";
  const derived: DerivedContactIdentity = {
    name: isLikelyPersonName(trimmedName)
      ? trimmedName
      : nameFromEmailLocalPart(email) || email,
  };
  const companyName = inferCompanyNameFromEmail(email);
  if (companyName) {
    derived.companyName = companyName;
  }
  return derived;
}

export function isLikelyPersonName(value: string): boolean {
  if (!value || value.length < 2 || value.length > 80) {
    return false;
  }

  if (value.includes("@") || /^https?:\/\//i.test(value)) {
    return false;
  }

  const normalized = normalizeName(value);
  if (
    !normalized ||
    [
      "what",
      "who",
      "invitee timezone",
      "meeting link",
      "zoom",
      "google meet",
      "teams",
    ].includes(normalized)
  ) {
    return false;
  }

  return (value.match(/\p{L}/gu)?.length ?? 0) >= 2;
}

export function nameFromEmailLocalPart(email: string): string {
  const local = email.split("@")[0]?.split("+")[0] ?? "";
  return local
    .replace(/[._-]+/g, " ")
    .split(" ")
    .map((part) => part.trim())
    .filter((part) => part.length > 0 && !/^\d+$/.test(part))
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1).toLowerCase())
    .join(" ");
}

export function inferCompanyNameFromEmail(
  email: string | undefined,
): string | undefined {
  const domain = email?.split("@")[1]?.toLowerCase();
  if (!domain || PERSONAL_EMAIL_DOMAINS.has(domain)) {
    return undefined;
  }

  const labels = domain.split(".").filter(Boolean);
  if (labels.length < 2) {
    return undefined;
  }

  const secondLast = labels[labels.length - 2];
  const companyLabel =
    labels.length >= 3 &&
    secondLast &&
    ["co", "com", "org", "net", "ac"].includes(secondLast)
      ? labels[labels.length - 3]
      : secondLast;
  if (!companyLabel || companyLabel.length < 2) {
    return undefined;
  }

  return normalizeCompanyName(
    companyLabel.charAt(0).toUpperCase() + companyLabel.slice(1),
  );
}

export function normalizeCompanyName(
  value: string | undefined | null,
): string | undefined {
  const name = value?.trim().replace(/\s+/g, " ");
  if (!name || name.length < 2 || name.length > 80) {
    return undefined;
  }

  if (name.includes("@") || /^https?:\/\//i.test(name)) {
    return undefined;
  }

  return name;
}

export function normalizeName(value: string): string {
  return value
    .normalize("NFKD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
}
