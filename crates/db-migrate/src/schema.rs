use hypr_db_core2::DbOpenOptions;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationFailurePolicy {
    Fail,
    Recreate,
}

#[derive(Clone, Copy, Debug)]
pub struct AppDbOpenOptions<'a> {
    pub db: DbOpenOptions<'a>,
    pub migration_failure_policy: MigrationFailurePolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationScope {
    Plain,
    CloudsyncAlter { table_name: &'static str },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MigrationStep {
    pub id: &'static str,
    pub scope: MigrationScope,
    pub sql: &'static str,
}

#[derive(Clone, Copy)]
pub struct DbSchema {
    pub steps: &'static [MigrationStep],
    pub validate_cloudsync_table: fn(&str) -> bool,
}
