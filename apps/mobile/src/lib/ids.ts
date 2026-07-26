import * as Crypto from "expo-crypto";

export const DEFAULT_USER_ID = "00000000-0000-0000-0000-000000000000";

export const id = (): string => Crypto.randomUUID();

export const nowIso = (): string => new Date().toISOString();
