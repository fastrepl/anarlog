pub(super) fn cloudsync_send_completed(result: &anlg_db_core::CloudsyncNetworkResult) -> bool {
    let Some(send) = result.send.as_ref() else {
        return false;
    };
    send.status.eq_ignore_ascii_case("synced") && send.last_failure.is_none()
}

pub(super) fn cloudsync_send_made_progress(result: &anlg_db_core::CloudsyncNetworkResult) -> bool {
    result
        .send
        .as_ref()
        .is_some_and(|send| send.chunks > 0 && send.last_failure.is_none())
}

#[cfg(test)]
pub(super) fn cloudsync_receive_completed(result: &anlg_db_core::CloudsyncNetworkResult) -> bool {
    result.receive.as_ref().is_some_and(|receive| {
        receive.complete && receive.error.is_none() && receive.last_failure.is_none()
    })
}

pub(super) fn cloudsync_receive_delivered(result: &anlg_db_core::CloudsyncNetworkResult) -> bool {
    result.receive.as_ref().is_some_and(|receive| {
        receive.chunks > 0 && receive.error.is_none() && receive.last_failure.is_none()
    })
}

#[cfg(test)]
fn cloudsync_receive_incomplete(result: &anlg_db_core::CloudsyncNetworkResult) -> bool {
    result.receive.as_ref().is_some_and(|receive| {
        !receive.complete && receive.error.is_none() && receive.last_failure.is_none()
    })
}

#[cfg(test)]
pub(super) fn cloudsync_receive_requires_reconciliation(
    result: &anlg_db_core::CloudsyncNetworkResult,
) -> bool {
    result
        .receive
        .as_ref()
        .is_some_and(|receive| receive.chunks > 0)
        || cloudsync_receive_incomplete(result)
}

pub(super) fn cloudsync_receive_delivered_final(
    result: &anlg_db_core::CloudsyncNetworkResult,
) -> bool {
    cloudsync_receive_delivered(result)
        && result
            .receive
            .as_ref()
            .is_some_and(|receive| receive.complete)
}
