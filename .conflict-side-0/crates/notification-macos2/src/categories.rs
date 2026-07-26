use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Mutex;

use objc2_foundation::{NSArray, NSString, ns_string};
use objc2_user_notifications::{
    UNNotificationAction, UNNotificationActionOptions, UNNotificationCategory,
    UNNotificationCategoryOptions, UNUserNotificationCenter,
};

/// Tracks registered category IDs and, for options categories, the labels needed to rebuild them.
static REGISTERED: Mutex<Option<HashMap<String, Vec<String>>>> = Mutex::new(None);

pub(crate) fn register_default(center: &UNUserNotificationCenter) {
    let mut reg = REGISTERED.lock().unwrap();
    let map = reg.get_or_insert_with(HashMap::new);
    if map.contains_key("HYPR_DEFAULT") {
        return;
    }
    map.insert("HYPR_DEFAULT".into(), vec![]);
    apply_all(center, map);
}

pub(crate) fn ensure_options_category_with_labels(
    center: &UNUserNotificationCenter,
    options: &[String],
) -> String {
    let mut hasher = DefaultHasher::new();
    options.hash(&mut hasher);
    let cat_id = format!("HYPR_OPTS_{:x}", hasher.finish());

    let mut reg = REGISTERED.lock().unwrap();
    let map = reg.get_or_insert_with(HashMap::new);
    if map.contains_key(&cat_id) {
        return cat_id;
    }
    map.insert(cat_id.clone(), options.to_vec());
    apply_all(center, map);
    cat_id
}

fn apply_all(center: &UNUserNotificationCenter, map: &HashMap<String, Vec<String>>) {
    let cats: Vec<_> = map
        .iter()
        .map(|(id, labels)| {
            if id == "HYPR_DEFAULT" {
                build_default_category()
            } else {
                build_options_category(id, labels)
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
        ns_string!("HYPR_DEFAULT"),
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
