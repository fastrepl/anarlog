create table if not exists "stripe"."products" (
  "id" text primary key,
  "object" text,
  "active" boolean,
  "description" text,
  "metadata" jsonb,
  "name" text,
  "created" integer,
  "images" jsonb,
  "livemode" boolean,
  "package_dimensions" jsonb,
  "shippable" boolean,
  "statement_descriptor" text,
  "unit_label" text,
  "updated" integer,
  "url" text
);
