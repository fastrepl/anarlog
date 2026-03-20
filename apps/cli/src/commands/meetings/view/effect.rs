pub(crate) enum Effect {
    SaveMemo { meeting_id: String, memo: String },
    Exit,
}
