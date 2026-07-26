alter table stripe.customers
    add deleted boolean default false not null;