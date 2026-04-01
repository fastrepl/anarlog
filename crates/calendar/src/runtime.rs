pub trait CalendarRuntime: Send + Sync + 'static {
    fn emit_changed(&self);
}
