create table if not exists "stripe".events (
    id text primary key,
    object text,
    data jsonb,
    type text,
    created integer,
    request text,
    updated integer,
    livemode boolean,
    api_version text,
    pending_webhooks bigint
);
