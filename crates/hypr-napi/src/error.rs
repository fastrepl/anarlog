pub(crate) fn to_napi_error(error: impl ToString) -> napi::Error {
    napi::Error::from_reason(error.to_string())
}
