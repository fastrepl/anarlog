create or replace function set_updated_at() returns trigger
    language plpgsql
as
$$
begin
  new.updated_at = now();
  return NEW;
end;
$$;

alter function set_updated_at() owner to postgres;

alter table stripe.subscriptions
    add updated_at timestamptz default timezone('utc'::text, now()) not null;

create trigger handle_updated_at
    before update
    on stripe.subscriptions
    for each row
    execute procedure set_updated_at();

alter table stripe.products
    add updated_at timestamptz default timezone('utc'::text, now()) not null;

create trigger handle_updated_at
    before update
    on stripe.products
    for each row
    execute procedure set_updated_at();

alter table stripe.customers
    add updated_at timestamptz default timezone('utc'::text, now()) not null;

create trigger handle_updated_at
    before update
    on stripe.customers
    for each row
    execute procedure set_updated_at();

alter table stripe.prices
    add updated_at timestamptz default timezone('utc'::text, now()) not null;

create trigger handle_updated_at
    before update
    on stripe.prices
    for each row
    execute procedure set_updated_at();

alter table stripe.invoices
    add updated_at timestamptz default timezone('utc'::text, now()) not null;

create trigger handle_updated_at
    before update
    on stripe.invoices
    for each row
    execute procedure set_updated_at();

alter table stripe.charges
    add updated_at timestamptz default timezone('utc'::text, now()) not null;

create trigger handle_updated_at
    before update
    on stripe.charges
    for each row
    execute procedure set_updated_at();

alter table stripe.coupons
    add updated_at timestamptz default timezone('utc'::text, now()) not null;

create trigger handle_updated_at
    before update
    on stripe.coupons
    for each row
    execute procedure set_updated_at();

alter table stripe.disputes
    add updated_at timestamptz default timezone('utc'::text, now()) not null;

create trigger handle_updated_at
    before update
    on stripe.disputes
    for each row
    execute procedure set_updated_at();

alter table stripe.events
    add updated_at timestamptz default timezone('utc'::text, now()) not null;

create trigger handle_updated_at
    before update
    on stripe.events
    for each row
    execute procedure set_updated_at();

alter table stripe.payouts
    add updated_at timestamptz default timezone('utc'::text, now()) not null;

create trigger handle_updated_at
    before update
    on stripe.payouts
    for each row
    execute procedure set_updated_at();

alter table stripe.plans
    add updated_at timestamptz default timezone('utc'::text, now()) not null;

create trigger handle_updated_at
    before update
    on stripe.plans
    for each row
    execute procedure set_updated_at();
