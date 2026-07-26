do $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'subscription_schedule_status') THEN
        create type "stripe"."subscription_schedule_status" as enum ('not_started', 'active', 'completed', 'released', 'canceled');
    END IF;
END
$$;

create table if not exists
    "stripe"."subscription_schedules" (
        id text primary key,
        object text,
        application text,
        canceled_at integer,
        completed_at integer,
        created integer not null,
        current_phase jsonb,
        customer text not null,
        default_settings jsonb,
        end_behavior text,
        livemode boolean not null,
        metadata jsonb not null,
        phases jsonb not null,
        released_at integer,
        released_subscription text,
        status stripe.subscription_schedule_status not null,
        subscription text,
        test_clock text
    );