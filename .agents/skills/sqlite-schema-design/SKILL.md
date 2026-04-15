---
name: sqlite-schema-design
description: Design or review schemas for `crates/cloudsync` using SQLite Sync constraints, not generic SQLite advice. Use when adding synced tables, changing synced columns, or planning CloudSync-safe migrations.
---

## Goal

Design tables that behave correctly under SQLite Sync's CRDT replication model.

This skill is specifically for CloudSync-backed schemas:

- tables are initialized through `cloudsync_init(...)`
- sync is enabled with `cloudsync_enable(...)`
- identifiers should be generated with `cloudsync_uuid()`
- schema changes must go through `cloudsync_begin_alter(...)` and `cloudsync_commit_alter(...)`
- local and cloud databases must keep the same schema

Do not treat this as ordinary SQLite schema design. SQLite Sync imposes extra rules around keys, defaults, foreign keys, and schema evolution.

## Workflow

### 1. Decide Whether The Table Is Synced

Before proposing DDL, classify the table:

- synced application data: must satisfy SQLite Sync constraints
- local-only cache or ephemeral state: should usually stay out of CloudSync

Only apply this skill to synced tables or to tables that may become synced soon.

### 2. Require A Stable, Globally Unique Primary Key

For synced tables:

- always declare an explicit primary key
- prefer `TEXT PRIMARY KEY NOT NULL`
- generate ids with `cloudsync_uuid()`
- do not use auto-incrementing integer ids

SQLite Sync docs explicitly recommend UUIDv7-style globally unique ids for CRDT workloads. Integer autoincrement ids are a bad fit because multiple devices can create rows independently.

```sql
CREATE TABLE document (
  id TEXT PRIMARY KEY NOT NULL DEFAULT (cloudsync_uuid()),
  workspace_id TEXT NOT NULL,
  title TEXT NOT NULL DEFAULT '',
  body TEXT NOT NULL DEFAULT '',
  archived INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL DEFAULT (unixepoch()),
  updated_at INTEGER NOT NULL DEFAULT (unixepoch())
) STRICT;
```

If the environment cannot use a function call in `DEFAULT`, generate the id in application code, but still use `cloudsync_uuid()` as the canonical id strategy.

### 3. Make Inserts Merge-Safe With Real Defaults

SQLite Sync best practices call out a non-obvious CRDT constraint: merges can happen column-by-column, so missing values are much more dangerous than in a single-node SQLite app.

For synced tables:

- every non-primary-key `NOT NULL` column should have a meaningful `DEFAULT`
- avoid required columns that only application code knows how to populate
- prefer simple scalar defaults over nullable columns when the field is logically always present

Good:

```sql
title TEXT NOT NULL DEFAULT ''
archived INTEGER NOT NULL DEFAULT 0
sort_order INTEGER NOT NULL DEFAULT 0
```

Bad:

```sql
title TEXT NOT NULL
archived INTEGER NOT NULL
```

without defaults on a synced table.

### 4. Keep The Local And Cloud Schemas Identical

The getting-started docs require the local synced database and the SQLite Cloud database to share the same schema.

When designing or reviewing a schema:

- treat local and remote DDL as one contract
- do not introduce "client-only" columns on synced tables
- do not rely on drift being harmless
- ensure migrations are applied consistently before sync resumes

If a field is only needed locally, it likely belongs in a separate non-synced table.

### 5. Be Conservative With Foreign Keys

SQLite Sync best practices explicitly warn that foreign keys can interact poorly with CRDT replication.

Use foreign keys on synced tables only when the integrity guarantee is worth the operational cost.

If you keep them:

- make child foreign key columns nullable when the relationship is optional
- if a foreign key column has a `DEFAULT`, that default must be `NULL` or reference an actually valid parent row
- avoid fake sentinel ids such as `'root'` unless that parent row is guaranteed to exist everywhere
- index the child foreign key columns

Prefer ownership patterns that tolerate out-of-order arrival between related rows.

### 6. Avoid Triggers And Implicit Write Logic On Synced Tables

SQLite Sync best practices advise minimizing triggers because they make replicated writes harder to reason about.

For synced tables:

- avoid triggers that mutate synced columns
- avoid hidden side effects on insert or update
- prefer explicit application writes
- keep derived or bookkeeping writes in non-synced tables if possible

If a trigger is unavoidable, review it as part of the replication design, not as a local SQLite convenience.

### 7. Scope Uniqueness For RLS And Multi-Tenant Sync

The introduction docs emphasize row-level security and multi-tenant access patterns.

That changes uniqueness design:

- if the real rule is "unique per workspace/user/team", encode that as a composite constraint
- avoid globally unique business keys unless they truly span all tenants

Prefer:

```sql
UNIQUE (workspace_id, slug)
```

over:

```sql
slug TEXT UNIQUE
```

when data is tenant-scoped.

### 8. Keep Schema Changes Inside The CloudSync Alter Window

Schema changes for synced databases are not ordinary `ALTER TABLE` work. Use:

1. `cloudsync_begin_alter('table_name')`
2. perform the schema change
3. `cloudsync_commit_alter('table_name')`

Design implications:

- favor additive changes over destructive rewrites
- prefer adding columns with safe defaults
- avoid migrations that temporarily violate sync invariants
- plan rollouts so every replica can move cleanly to the new shape

When reviewing a migration plan, reject any synced-table schema change that skips the CloudSync alter flow.

### 9. Keep Sync Metadata Out Of Your Domain Schema

SQLite Sync already exposes its own metadata and helpers:

- `cloudsync_siteid()`
- `cloudsync_db_version()`
- `cloudsync_version()`
- `cloudsync_is_enabled()`

Do not duplicate these concepts as app-managed columns on synced tables unless there is a very specific product requirement.

### 10. Separate Network Lifecycle From Schema Design

The API set includes network setup and sync transport functions such as:

- `cloudsync_network_init(...)`
- `cloudsync_network_set_token(...)`
- `cloudsync_network_set_apikey(...)`
- `cloudsync_network_sync(...)`
- `cloudsync_network_has_unsent_changes()`

These matter operationally, but they are not substitutes for sound schema design.

Do not design tables that assume:

- sync is always online
- rows arrive in lockstep
- dependent rows replicate in a single transaction boundary visible to all peers

Assume offline creation, delayed delivery, retries, and independent merges.

## Design Defaults For Synced Tables

Unless the user explicitly asks otherwise:

- `TEXT PRIMARY KEY NOT NULL`
- ids generated with `cloudsync_uuid()`
- `STRICT` tables
- explicit `DEFAULT` on every non-key `NOT NULL` column
- composite uniqueness for tenant-scoped identifiers
- minimal or no triggers
- cautious foreign key usage
- separate non-synced tables for local UI/cache state

## Review Checklist

When reviewing a CloudSync schema, ask:

- Does every synced table have a globally unique primary key strategy?
- Are new rows creatable independently on multiple devices?
- Do all non-key required columns have defaults that make replicated inserts safe?
- Would the schema still behave correctly if related rows arrive out of order?
- Are foreign keys optional where replication ordering can vary?
- Are tenant-scoped uniqueness rules modeled as composite constraints?
- Does the migration plan use `cloudsync_begin_alter` / `cloudsync_commit_alter`?
- Is any local-only state incorrectly mixed into a synced table?
