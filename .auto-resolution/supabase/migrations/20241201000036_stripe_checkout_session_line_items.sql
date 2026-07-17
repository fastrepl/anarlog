create table if not exists "stripe"."checkout_session_line_items" (
  "id" text primary key,
  "object" text,
  "amount_discount" integer,
  "amount_subtotal" integer,
  "amount_tax" integer,
  "amount_total" integer,
  "currency" text,
  "description" text,
  "price" text references "stripe"."prices" on delete cascade,
  "quantity" integer,
  "checkout_session" text references "stripe"."checkout_sessions" on delete cascade,
  "updated_at" timestamptz default timezone('utc'::text, now()) not null,
  "last_synced_at" timestamptz
);

create index stripe_checkout_session_line_items_session_idx on "stripe"."checkout_session_line_items" using btree (checkout_session);
create index stripe_checkout_session_line_items_price_idx on "stripe"."checkout_session_line_items" using btree (price);

create trigger handle_updated_at
    before update
    on stripe.checkout_session_line_items
    for each row
    execute procedure set_updated_at(); 