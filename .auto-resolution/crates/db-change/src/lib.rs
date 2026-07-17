mod notifier;
mod tracker;

pub use notifier::ChangeNotifier;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TableChangeKind {
    Insert,
    Update,
    Delete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableChange {
    pub table: String,
    pub kind: TableChangeKind,
    pub seq: u64,
}
