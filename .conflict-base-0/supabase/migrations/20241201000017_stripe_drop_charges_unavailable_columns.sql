-- drop columns that are duplicated / not available anymore
-- card is not available on webhook v.2020-03-02. We can get the detail from payment_method_details
-- statement_description is not available on webhook v.2020-03-02
alter table "stripe"."charges"
    drop column if exists "card",
    drop column if exists "statement_description";