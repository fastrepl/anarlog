use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSPasteboard, NSPasteboardItem, NSPasteboardTypeString, NSPasteboardWriting};
use objc2_foundation::{NSArray, NSData, NSString};

#[derive(Debug, Clone, PartialEq, Eq)]
struct PasteboardEntry {
    type_name: String,
    data: Vec<u8>,
}

/// An owned snapshot of every pasteboard item and representation.
///
/// The snapshot contains no AppKit objects, so it can be moved to a worker
/// thread and restored later on the main thread.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PasteboardSnapshot {
    items: Vec<Vec<PasteboardEntry>>,
}

impl PasteboardSnapshot {
    pub fn capture(pasteboard: &NSPasteboard) -> Self {
        let Some(items) = pasteboard.pasteboardItems() else {
            return Self::default();
        };
        let items = items
            .iter()
            .map(|item| {
                item.types()
                    .iter()
                    .filter_map(|type_name| {
                        item.dataForType(&type_name).map(|data| PasteboardEntry {
                            type_name: type_name.to_string(),
                            data: data.to_vec(),
                        })
                    })
                    .collect()
            })
            .filter(|entries: &Vec<PasteboardEntry>| !entries.is_empty())
            .collect();
        Self { items }
    }

    /// Replace the pasteboard with this snapshot.
    pub fn restore(&self, pasteboard: &NSPasteboard) -> bool {
        pasteboard.clearContents();
        if self.items.is_empty() {
            return true;
        }

        let items: Vec<Retained<NSPasteboardItem>> = self
            .items
            .iter()
            .map(|entries| {
                let item = NSPasteboardItem::new();
                for entry in entries {
                    let type_name = NSString::from_str(&entry.type_name);
                    let data = NSData::with_bytes(&entry.data);
                    item.setData_forType(&data, &type_name);
                }
                item
            })
            .collect();
        let writers: Vec<&ProtocolObject<dyn NSPasteboardWriting>> = items
            .iter()
            .map(|item| {
                let item: &NSPasteboardItem = item;
                ProtocolObject::from_ref(item)
            })
            .collect();
        pasteboard.writeObjects(&NSArray::from_slice(&writers))
    }

    /// Restore only while the temporary pasteboard contents are still current.
    ///
    /// Passing an expected string additionally verifies the current public
    /// string representation. Passing `None` relies only on the change count.
    pub fn restore_if_unchanged(
        &self,
        pasteboard: &NSPasteboard,
        expected_change_count: isize,
        expected_string: Option<&str>,
    ) -> bool {
        let current_string_matches = expected_string.is_none_or(|expected| {
            pasteboard
                .stringForType(unsafe { NSPasteboardTypeString })
                .is_some_and(|current| current.to_string() == expected)
        });
        if !restore_conditions_met(
            pasteboard.changeCount(),
            expected_change_count,
            current_string_matches,
        ) {
            return false;
        }
        self.restore(pasteboard)
    }
}

fn restore_conditions_met(
    current_change_count: isize,
    expected_change_count: isize,
    current_string_matches: bool,
) -> bool {
    current_change_count == expected_change_count && current_string_matches
}

#[cfg(test)]
mod tests {
    use super::restore_conditions_met;

    #[test]
    fn restoration_requires_unchanged_expected_contents() {
        assert!(restore_conditions_met(4, 4, true));
        assert!(!restore_conditions_met(5, 4, true));
        assert!(!restore_conditions_met(4, 4, false));
    }
}
