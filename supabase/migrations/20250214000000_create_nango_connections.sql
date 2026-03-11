CREATE TABLE "nango_connections" (
    "id" uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    "user_id" uuid NOT NULL,
    "integration_id" text NOT NULL,
    "connection_id" text NOT NULL,
    "provider" text NOT NULL,
    "created_at" timestamptz NOT NULL DEFAULT now(),
    "updated_at" timestamptz NOT NULL DEFAULT now()
);

ALTER TABLE "nango_connections" ENABLE ROW LEVEL SECURITY;

ALTER TABLE "nango_connections"
    ADD CONSTRAINT "nango_connections_user_id_fk"
    FOREIGN KEY ("user_id") REFERENCES "auth"."users"("id") ON DELETE CASCADE;

CREATE UNIQUE INDEX "nango_connections_user_integration_idx"
    ON "nango_connections" ("user_id", "integration_id");

CREATE INDEX "nango_connections_connection_id_idx"
    ON "nango_connections" ("connection_id");

CREATE POLICY "nango_connections_select_owner" ON "nango_connections"
    AS PERMISSIVE FOR SELECT TO "authenticated"
    USING ((select auth.uid()) = user_id);

CREATE POLICY "nango_connections_service_all" ON "nango_connections"
    AS PERMISSIVE FOR ALL TO "service_role"
    USING (true) WITH CHECK (true);
