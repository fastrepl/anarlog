begin;
select plan(8);

select tests.create_supabase_user('addon_pro', 'addon-pro@example.com');

update public.profiles
set stripe_customer_id = 'cus_addon_pro'
where id = tests.get_supabase_uid('addon_pro');

insert into stripe.customers (id) values ('cus_addon_pro') on conflict (id) do nothing;
insert into stripe.prices (id, lookup_key)
values
  ('price_addon_monthly', 'hyprnote_sync_device_addon_monthly'),
  ('price_addon_yearly', 'hyprnote_sync_device_addon_yearly'),
  ('price_pro_monthly', 'hyprnote_pro_monthly_test')
on conflict (id) do nothing;
insert into stripe.subscriptions (id, customer, status)
values ('sub_addon_pro', 'cus_addon_pro', 'active')
on conflict (id) do nothing;
insert into stripe.subscription_items (id, subscription, price, quantity)
values ('si_addon_pro_base', 'sub_addon_pro', 'price_pro_monthly', 1)
on conflict (id) do nothing;

select is(public.get_sync_device_limit(tests.get_supabase_uid('addon_pro')), 3,
  'A Pro subscription without add-ons keeps three included slots');

insert into stripe.subscription_items (id, subscription, price, quantity)
values ('si_addon_pro_devices', 'sub_addon_pro', 'price_addon_monthly', 2);

select is(public.get_sync_device_limit(tests.get_supabase_uid('addon_pro')), 5,
  'Each add-on unit adds one sync device slot');

update stripe.subscription_items set quantity = 4 where id = 'si_addon_pro_devices';
select is(public.get_sync_device_limit(tests.get_supabase_uid('addon_pro')), 7,
  'Add-on quantity changes update the allowance');

update stripe.subscription_items set deleted = true where id = 'si_addon_pro_devices';
select is(public.get_sync_device_limit(tests.get_supabase_uid('addon_pro')), 3,
  'Removed add-on items no longer grant slots');

update stripe.subscription_items set deleted = false, price = 'price_addon_yearly' where id = 'si_addon_pro_devices';
select is(public.get_sync_device_limit(tests.get_supabase_uid('addon_pro')), 7,
  'Yearly add-on prices count the same way');

update stripe.subscriptions set status = 'canceled' where id = 'sub_addon_pro';
select is(public.get_sync_device_limit(tests.get_supabase_uid('addon_pro')), 3,
  'Add-ons on a canceled subscription do not grant slots');

update stripe.subscriptions set status = 'active' where id = 'sub_addon_pro';
select tests.authenticate_as_service_role();
select public.claim_personal_workspace_e2ee_key(tests.get_supabase_uid('addon_pro'), 'abcdefghijklmnopqrstuv');
do $$begin
  for ordinal in 1..6 loop
    perform * from public.claim_sync_device(tests.get_supabase_uid('addon_pro'), 'addon-device-' || ordinal::text);
  end loop;
end$$;
select is((select allowed from public.claim_sync_device(tests.get_supabase_uid('addon_pro'), 'addon-device-7')), true,
  'Purchased slots can be claimed');
select is((select allowed from public.claim_sync_device(tests.get_supabase_uid('addon_pro'), 'addon-device-8')), false,
  'Claims stop at the purchased allowance');

select * from finish();
rollback;
