create extension if not exists http with schema extensions;
create extension if not exists pg_tle;
drop extension if exists "supabase-dbdev";
select pgtle.uninstall_extension_if_exists('supabase-dbdev');
select
    pgtle.install_extension(
        'supabase-dbdev',
        resp.contents ->> 'version',
        'PostgreSQL package manager',
        resp.contents ->> 'sql'
    )
from extensions.http(
    (
        'GET',
        'https://api.database.dev/rest/v1/'
        || 'package_versions?select=sql,version'
        || '&package_name=eq.supabase-dbdev'
        || '&order=version.desc'
        || '&limit=1',
        array[
            ('apiKey', 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6InhtdXB0cHBsZnZpaWZyYndtbXR2Iiwicm9sZSI6ImFub24iLCJpYXQiOjE2ODAxMDczNzIsImV4cCI6MTk5NTY4MzM3Mn0.z2CN0mvO2No8wSi46Gw59DFGCTJrzM0AQKsu_5k134s')::extensions.http_header
        ],
        null,
        null
    )
) x,
lateral (
    select
        ((row_to_json(x) -> 'content') #>> '{}')::json -> 0
) resp(contents);
create extension "supabase-dbdev";
select dbdev.install('supabase-dbdev');

-- Drop and recreate the extension to ensure a clean installation
drop extension if exists "supabase-dbdev";
create extension "supabase-dbdev";

-- Install test helpers package
select dbdev.install('basejump-supabase_test_helpers');
create extension if not exists "basejump-supabase_test_helpers" version '0.0.6';

create or replace function tests.authenticate_as_hyprnote_pro(identifier text)
returns void
language plpgsql
set search_path = ''
as $$
declare
    claims jsonb;
begin
    perform tests.authenticate_as(identifier);
    claims := coalesce(
        current_setting('request.jwt.claims', true)::jsonb,
        '{}'::jsonb
    );
    perform set_config(
        'request.jwt.claims',
        jsonb_set(
            claims,
            '{entitlements}',
            '["hyprnote_pro"]'::jsonb,
            true
        )::text,
        true
    );
end;
$$;

begin;
select plan(1);

select results_eq(
    $$select count(*) from pg_extension where extname = 'supabase-dbdev'$$,
    array[1::bigint],
    'Installs supabase-dbdev extension exactly once'
);

select * from finish();
rollback;
