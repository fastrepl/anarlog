use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Mutex;

use objc2_foundation::{NSArray, NSString, ns_string};
use objc2_user_notifications::{
    UNNotificationAction, UNNotificationActionOptions, UNNotificationCategory,
    UNNotificationCategoryOptions, UNUserNotificationCenter,
};

const DEFAULT_CATEGORY_ID: &str = "ANLG_DEFAULT";
const MAX_OPTIONS_CATEGORIES: usize = 64;
const MAX_OPTIONS_PER_CATEGORY: usize = 4;
const MAX_OPTION_LABEL_BYTES: usize = 256;
const MAX_OPTION_LABELS_BYTES: usize = 512;

struct CategoryEntry {
    labels: Vec<String>,
    sequence: u64,
}

#[derive(Default)]
struct CategoryRegistry {
    entries: HashMap<String, CategoryEntry>,
    next_sequence: u64,
}

impl CategoryRegistry {
    fn touch(&mut self, id: &str) -> bool {
        let Some(entry) = self.entries.get_mut(id) else {
            return false;
        };
        self.next_sequence = self.next_sequence.wrapping_add(1);
        entry.sequence = self.next_sequence;
        true
    }

    fn insert(&mut self, id: String, labels: Vec<String>) {
        self.next_sequence = self.next_sequence.wrapping_add(1);
        if id != DEFAULT_CATEGORY_ID {
            while self
                .entries
                .keys()
                .filter(|key| key.as_str() != DEFAULT_CATEGORY_ID)
                .count()
                >= MAX_OPTIONS_CATEGORIES
            {
                let Some(oldest) = self
                    .entries
                    .iter()
                    .filter(|(key, _)| key.as_str() != DEFAULT_CATEGORY_ID)
                    .min_by_key(|(_, entry)| entry.sequence)
                    .map(|(key, _)| key.clone())
                else {
                    break;
                };
                self.entries.remove(&oldest);
            }
        }
        self.entries.insert(
            id,
            CategoryEntry {
                labels,
                sequence: self.next_sequence,
            },
        );
    }
}

/// Tracks registered category IDs and, for options categories, the labels needed to rebuild them.
static REGISTERED: Mutex<Option<CategoryRegistry>> = Mutex::new(None);

pub(crate) fn register_default(center: &UNUserNotificationCenter) {
    let mut reg = REGISTERED.lock().unwrap();
    let registry = reg.get_or_insert_with(CategoryRegistry::default);
    if registry.touch(DEFAULT_CATEGORY_ID) {
        return;
    }
    registry.insert(DEFAULT_CATEGORY_ID.into(), vec![]);
    apply_all(center, registry);
}

pub(crate) fn ensure_options_category_with_labels(
    center: &UNUserNotificationCenter,
    options: &[String],
) -> String {
    let labels = bound_option_labels(options);
    if labels.len() != options.len()
        || labels
            .iter()
            .zip(options)
            .any(|(bounded, original)| bounded != original)
    {
        log::warn!("notification options exceeded category bounds; using bounded action labels");
    }

    let mut hasher = DefaultHasher::new();
    labels.hash(&mut hasher);
    let cat_id = format!("ANLG_OPTS_{:x}", hasher.finish());

    let mut reg = REGISTERED.lock().unwrap();
    let registry = reg.get_or_insert_with(CategoryRegistry::default);
    if registry.touch(&cat_id) {
        return cat_id;
    }
    registry.insert(cat_id.clone(), labels);
    apply_all(center, registry);
    cat_id
}

fn bound_option_labels(options: &[String]) -> Vec<String> {
    let mut remaining_bytes = MAX_OPTION_LABELS_BYTES;
    let mut labels = Vec::with_capacity(options.len().min(MAX_OPTIONS_PER_CATEGORY));

    for option in options.iter().take(MAX_OPTIONS_PER_CATEGORY) {
        if remaining_bytes == 0 {
            break;
        }

        let byte_limit = remaining_bytes.min(MAX_OPTION_LABEL_BYTES);
        let bounded = truncate_utf8(option, byte_limit);
        if bounded.is_empty() && !option.is_empty() {
            break;
        }

        remaining_bytes -= bounded.len();
        labels.push(bounded.to_string());
    }

    labels
}

fn truncate_utf8(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }

    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

fn apply_all(center: &UNUserNotificationCenter, registry: &CategoryRegistry) {
    let cats: Vec<_> = registry
        .entries
        .iter()
        .map(|(id, entry)| {
            if id == DEFAULT_CATEGORY_ID {
                build_default_category()
            } else {
                build_options_category(id, &entry.labels)
            }
        })
        .collect();

    let set = objc2_foundation::NSSet::from_retained_slice(&cats);
    center.setNotificationCategories(&set);
}

fn build_default_category() -> objc2::rc::Retained<UNNotificationCategory> {
    let accept = UNNotificationAction::actionWithIdentifier_title_options(
        &NSString::from_str("ACCEPT"),
        ns_string!("Accept"),
        UNNotificationActionOptions::empty(),
    );
    UNNotificationCategory::categoryWithIdentifier_actions_intentIdentifiers_options(
        ns_string!("ANLG_DEFAULT"),
        &NSArray::from_retained_slice(&[accept]),
        &NSArray::from_slice(&[]),
        UNNotificationCategoryOptions::CustomDismissAction,
    )
}

fn build_options_category(
    cat_id: &str,
    labels: &[String],
) -> objc2::rc::Retained<UNNotificationCategory> {
    let actions: Vec<_> = labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            UNNotificationAction::actionWithIdentifier_title_options(
                &NSString::from_str(&format!("OPTION_{i}")),
                &NSString::from_str(label),
                UNNotificationActionOptions::empty(),
            )
        })
        .collect();
    UNNotificationCategory::categoryWithIdentifier_actions_intentIdentifiers_options(
        &NSString::from_str(cat_id),
        &NSArray::from_retained_slice(&actions),
        &NSArray::from_slice(&[]),
        UNNotificationCategoryOptions::CustomDismissAction,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn option_labels_are_limited_to_the_supported_action_count() {
        let options: Vec<_> = (0..MAX_OPTIONS_PER_CATEGORY + 2)
            .map(|index| format!("option-{index}"))
            .collect();

        let labels = bound_option_labels(&options);

        assert_eq!(labels.len(), MAX_OPTIONS_PER_CATEGORY);
        assert_eq!(labels, options[..MAX_OPTIONS_PER_CATEGORY]);
    }

    #[test]
    fn option_labels_are_truncated_at_utf8_boundaries() {
        let options = vec!["🙂".repeat(MAX_OPTION_LABEL_BYTES / 4 + 1)];

        let labels = bound_option_labels(&options);

        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].len(), MAX_OPTION_LABEL_BYTES);
        assert_eq!(labels[0], "🙂".repeat(MAX_OPTION_LABEL_BYTES / 4));
    }

    #[test]
    fn option_labels_respect_the_aggregate_utf8_byte_limit() {
        let first = "a".repeat(MAX_OPTION_LABEL_BYTES);
        let second = "b".repeat(MAX_OPTION_LABEL_BYTES - 5);
        let options = vec![
            first.clone(),
            second.clone(),
            "🙂🙂".to_string(),
            "unused".to_string(),
        ];

        let labels = bound_option_labels(&options);

        assert_eq!(
            labels,
            vec![first, second, "🙂".to_string(), "u".to_string()]
        );
        assert!(labels.iter().map(String::len).sum::<usize>() <= MAX_OPTION_LABELS_BYTES);
    }

    #[test]
    fn option_categories_evict_least_recently_used_entries() {
        let mut registry = CategoryRegistry::default();
        registry.insert(DEFAULT_CATEGORY_ID.to_string(), vec![]);
        for index in 0..MAX_OPTIONS_CATEGORIES {
            registry.insert(format!("category-{index}"), vec![index.to_string()]);
        }
        assert!(registry.touch("category-0"));

        registry.insert("category-new".to_string(), vec!["new".to_string()]);

        assert_eq!(registry.entries.len(), MAX_OPTIONS_CATEGORIES + 1);
        assert!(registry.entries.contains_key(DEFAULT_CATEGORY_ID));
        assert!(registry.entries.contains_key("category-0"));
        assert!(!registry.entries.contains_key("category-1"));
        assert!(registry.entries.contains_key("category-new"));
    }
}
