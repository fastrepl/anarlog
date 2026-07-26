create table
    if not exists "stripe"."features" (
        "id" text primary key,
        object text,
        livemode boolean,
        name text,
        lookup_key text unique,
        active boolean,
        metadata jsonb,
        updated_at timestamptz default timezone('utc'::text, now()) not null,
        last_synced_at timestamptz
    );

create trigger handle_updated_at
    before update
    on stripe.features
    for each row
    execute procedure set_updated_at();
