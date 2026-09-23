use std::path::Path;

const FILE_NAME: &str = "live-gaps.json";
const MAX_GAPS: usize = 128;
// Words can still be finalizing when the stream dies, so the gap starts a
// little before the last confirmed word.
const CONFIRMED_MARGIN_MS: u64 = 1_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LiveGap {
    pub start_ms: u64,
    pub end_ms: u64,
}

/// Capture time (ms since capture start) that live transcription did not
/// cover. Owned by the session supervisor and mirrored to disk so it survives
/// renderer reloads and outlives the session actors.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LiveGaps {
    pub capture_started_at: u64,
    pub closed: Vec<LiveGap>,
    pub open_since_ms: Option<u64>,
}

impl LiveGaps {
    pub fn new(capture_started_at: u64) -> Self {
        Self {
            capture_started_at,
            ..Self::default()
        }
    }

    pub fn open(&mut self, confirmed_through_ms: u64) -> bool {
        if self.open_since_ms.is_some() {
            return false;
        }
        self.open_since_ms = Some(confirmed_through_ms.saturating_sub(CONFIRMED_MARGIN_MS));
        true
    }

    pub fn close(&mut self, now_ms: u64) -> bool {
        let Some(start_ms) = self.open_since_ms.take() else {
            return false;
        };
        self.closed.push(LiveGap {
            start_ms,
            end_ms: now_ms.max(start_ms),
        });
        if self.closed.len() > MAX_GAPS {
            let first = self.closed[0].start_ms;
            let last = self.closed[self.closed.len() - 1].end_ms;
            self.closed = vec![LiveGap {
                start_ms: first,
                end_ms: last,
            }];
        }
        true
    }

    pub fn is_empty(&self) -> bool {
        self.closed.is_empty() && self.open_since_ms.is_none()
    }

    /// Gaps clipped to `[start_ms, end_ms)`, treating an open gap as extending
    /// to `end_ms`.
    pub fn within(&self, start_ms: u64, end_ms: u64) -> Vec<LiveGap> {
        self.closed
            .iter()
            .copied()
            .chain(self.open_since_ms.map(|start| LiveGap {
                start_ms: start,
                end_ms,
            }))
            .map(|gap| LiveGap {
                start_ms: gap.start_ms.max(start_ms),
                end_ms: gap.end_ms.min(end_ms),
            })
            .filter(|gap| gap.start_ms < gap.end_ms)
            .collect()
    }
}

pub fn write_live_gaps(session_dir: &Path, gaps: &LiveGaps) -> std::io::Result<()> {
    std::fs::create_dir_all(session_dir)?;
    let path = session_dir.join(FILE_NAME);
    let tmp = session_dir.join(format!("{FILE_NAME}.tmp"));
    std::fs::write(
        &tmp,
        serde_json::to_vec(gaps).map_err(std::io::Error::other)?,
    )?;
    std::fs::rename(tmp, path)
}

pub fn read_live_gaps(session_dir: &Path) -> std::io::Result<LiveGaps> {
    match std::fs::read(session_dir.join(FILE_NAME)) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(std::io::Error::other),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(LiveGaps::default()),
        Err(error) => Err(error),
    }
}

pub(super) fn remove_live_gaps(session_dir: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(session_dir.join(FILE_NAME)) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_starts_before_last_confirmed_word_and_close_ends_now() {
        let mut gaps = LiveGaps::default();
        assert!(gaps.open(30_000));
        assert!(!gaps.open(40_000), "an open gap stays open");
        assert_eq!(gaps.open_since_ms, Some(29_000));
        assert!(gaps.close(45_000));
        assert!(!gaps.close(50_000), "nothing to close");
        assert_eq!(
            gaps.closed,
            vec![LiveGap {
                start_ms: 29_000,
                end_ms: 45_000
            }]
        );
    }

    #[test]
    fn open_gap_extends_to_the_query_end() {
        let mut gaps = LiveGaps::default();
        gaps.open(500);
        assert_eq!(
            gaps.within(60_000, 120_000),
            vec![LiveGap {
                start_ms: 60_000,
                end_ms: 120_000
            }]
        );
        assert_eq!(gaps.within(0, 0), vec![]);
    }

    #[test]
    fn within_clips_to_the_chunk() {
        let mut gaps = LiveGaps::default();
        gaps.open(10_000);
        gaps.close(70_000);
        gaps.open(200_000);
        gaps.close(201_000);
        assert_eq!(
            gaps.within(0, 60_000),
            vec![LiveGap {
                start_ms: 9_000,
                end_ms: 60_000
            }]
        );
        assert_eq!(
            gaps.within(60_000, 120_000),
            vec![LiveGap {
                start_ms: 60_000,
                end_ms: 70_000
            }]
        );
        assert_eq!(gaps.within(120_000, 180_000), vec![]);
    }

    #[test]
    fn outage_count_is_bounded() {
        let mut gaps = LiveGaps::default();
        for i in 0..(MAX_GAPS as u64 + 10) {
            gaps.open(i * 10_000 + 1_000);
            gaps.close(i * 10_000 + 5_000);
        }
        assert!(gaps.closed.len() <= MAX_GAPS);
        assert_eq!(
            gaps.closed[0].start_ms, 0,
            "collapsed span keeps the first start"
        );
        assert_eq!(
            gaps.closed.last().unwrap().end_ms,
            (MAX_GAPS as u64 + 9) * 10_000 + 5_000
        );
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_live_gaps(dir.path()).unwrap(), LiveGaps::default());
        let mut gaps = LiveGaps::default();
        gaps.open(5_000);
        gaps.close(9_000);
        gaps.open(20_000);
        write_live_gaps(dir.path(), &gaps).unwrap();
        assert_eq!(read_live_gaps(dir.path()).unwrap(), gaps);
        remove_live_gaps(dir.path()).unwrap();
        assert_eq!(read_live_gaps(dir.path()).unwrap(), LiveGaps::default());
    }
}
