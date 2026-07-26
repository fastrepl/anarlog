ALTER TABLE if exists "stripe"."plans" DROP COLUMN name;
ALTER TABLE if exists "stripe"."plans" DROP COLUMN updated;
ALTER TABLE if exists "stripe"."plans" DROP COLUMN tiers;
ALTER TABLE if exists "stripe"."plans" DROP COLUMN statement_descriptor;
ALTER TABLE if exists "stripe"."plans" DROP COLUMN statement_description;