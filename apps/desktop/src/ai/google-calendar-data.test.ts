// @vitest-environment node

import { DatabaseSync } from "node:sqlite";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import {
  GOOGLE_CALENDAR_DATA_SQL,
  type GoogleCalendarDataSqlRow,
} from "./google-calendar-data";

describe("Google Calendar data query", () => {
  let database: DatabaseSync;

  beforeEach(() => {
    database = new DatabaseSync(":memory:");
    database.exec(`
      CREATE TABLE calendars (
        id TEXT NOT NULL DEFAULT '',
        provider TEXT NOT NULL DEFAULT '',
        deleted_at TEXT
      );
      CREATE TABLE events (
        id TEXT NOT NULL DEFAULT '',
        tracking_id_event TEXT NOT NULL DEFAULT '',
        calendar_id TEXT NOT NULL DEFAULT '',
        provider TEXT NOT NULL DEFAULT '',
        deleted_at TEXT
      );
      CREATE TABLE sessions (
        event_id TEXT NOT NULL DEFAULT '',
        external_event_id TEXT NOT NULL DEFAULT '',
        external_provider TEXT NOT NULL DEFAULT '',
        event_json TEXT NOT NULL DEFAULT '',
        deleted_at TEXT
      );
    `);
  });

  afterEach(() => {
    database.close();
  });

  function hasGoogleCalendarData(): boolean {
    const row = database
      .prepare(GOOGLE_CALENDAR_DATA_SQL)
      .get() as GoogleCalendarDataSqlRow;
    return Boolean(row.has_google_calendar_data);
  }

  it("returns false without calendar-derived data", () => {
    expect(hasGoogleCalendarData()).toBe(false);
  });

  it("includes live and tombstoned Google provider rows", () => {
    database.exec(`
      INSERT INTO calendars (provider) VALUES ('google');
    `);
    expect(hasGoogleCalendarData()).toBe(true);

    database.exec("DELETE FROM calendars;");
    database.exec(`
      INSERT INTO events (id, provider, deleted_at)
      VALUES ('event-1', 'google-calendar', '2026-07-20T00:00:00Z');
    `);
    expect(hasGoogleCalendarData()).toBe(true);

    database.exec("DELETE FROM events;");
    database.exec(`
      INSERT INTO sessions (external_provider, deleted_at)
      VALUES ('google', '2026-07-20T00:00:00Z');
    `);
    expect(hasGoogleCalendarData()).toBe(true);
  });

  it("fails closed for legacy event-backed sessions without provenance", () => {
    database.exec(`
      INSERT INTO sessions (event_json)
      VALUES ('{"tracking_id":"event-1","calendar_id":"calendar-1","title":"Imported calendar event"}');
    `);

    expect(hasGoogleCalendarData()).toBe(true);
  });

  it("does not classify the local welcome event as Google data", () => {
    database.exec(`
      INSERT INTO sessions (event_json)
      VALUES ('{"tracking_id":"anarlog-onboarding-demo-v1","calendar_id":"","title":"Welcome to Anarlog"}');
    `);

    expect(hasGoogleCalendarData()).toBe(false);
  });

  it("does not classify a legacy session linked to a known Outlook event as Google data", () => {
    database.exec(`
      INSERT INTO events (id, tracking_id_event, provider)
      VALUES ('event-1', 'outlook-1', 'outlook');
      INSERT INTO sessions (event_id, external_event_id, event_json)
      VALUES ('event-1', 'outlook-1', '{"title":"Outlook event"}');
    `);

    expect(hasGoogleCalendarData()).toBe(false);
  });

  it("does not classify an orphaned session linked to a known Outlook calendar as Google data", () => {
    database.exec(`
      INSERT INTO calendars (id, provider)
      VALUES ('outlook-calendar-1', 'outlook');
      INSERT INTO sessions (event_json)
      VALUES ('{"calendar_id":"outlook-calendar-1","title":"Outlook event"}');
    `);

    expect(hasGoogleCalendarData()).toBe(false);
  });

  it("fails closed when an external event ID lacks calendar-scoped provenance", () => {
    database.exec(`
      INSERT INTO events (tracking_id_event, calendar_id, provider)
      VALUES ('event-1', 'outlook-calendar-1', 'outlook');
      INSERT INTO sessions (external_event_id, event_json)
      VALUES ('event-1', '{"title":"Imported event"}');
    `);

    expect(hasGoogleCalendarData()).toBe(true);
  });
});
