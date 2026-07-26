use crate::proto::{MergableDataProto, MergeableDataObjectEntry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const LEFT_TO_RIGHT_DIRECTION: &str = "CRTableColumnDirectionLeftToRight";
const RIGHT_TO_LEFT_DIRECTION: &str = "CRTableColumnDirectionRightToLeft";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub rows: Vec<Vec<String>>,
    pub direction: String,
}

impl Table {
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            direction: LEFT_TO_RIGHT_DIRECTION.to_string(),
        }
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    pub fn column_count(&self) -> usize {
        self.rows.first().map(|r| r.len()).unwrap_or(0)
    }
}

impl Default for Table {
    fn default() -> Self {
        Self::new()
    }
}

fn get_target_uuid_from_object_entry(object_entry: &MergeableDataObjectEntry) -> Option<u64> {
    object_entry
        .custom_map
        .as_ref()?
        .map_entry
        .first()
        .map(|entry| entry.value.unsigned_integer_value)
}

pub fn parse_table(proto: &MergableDataProto) -> Option<Table> {
    let data = &proto.mergable_data_object.mergeable_data_object_data;

    let key_items = &data.mergeable_data_object_key_item;
    let type_items = &data.mergeable_data_object_type_item;
    let uuid_items = &data.mergeable_data_object_uuid_item;
    let table_objects = &data.mergeable_data_object_entry;

    let mut table_direction = LEFT_TO_RIGHT_DIRECTION.to_string();

    for entry in table_objects {
        if let Some(ref custom_map) = entry.custom_map
            && let Some(first_entry) = custom_map.map_entry.first()
            && (first_entry.key as usize) == key_items.len()
            && let Some(dir) = &first_entry.value.string_value.as_str().into()
        {
            table_direction = (*dir).to_string();
        }
    }

    for entry in table_objects {
        if let Some(ref custom_map) = entry.custom_map {
            let type_index = custom_map.r#type as usize;
            if type_index < type_items.len() && type_items[type_index] == "com.apple.notes.ICTable"
            {
                return parse_table_entry(
                    entry,
                    key_items,
                    uuid_items,
                    table_objects,
                    &table_direction,
                );
            }
        }
    }

    None
}

fn parse_table_entry(
    table_entry: &MergeableDataObjectEntry,
    key_items: &[String],
    uuid_items: &[Vec<u8>],
    table_objects: &[MergeableDataObjectEntry],
    table_direction: &str,
) -> Option<Table> {
    let custom_map = table_entry.custom_map.as_ref()?;

    let mut row_indices: HashMap<u64, usize> = HashMap::new();
    let mut column_indices: HashMap<u64, usize> = HashMap::new();
    let mut total_rows = 0;
    let mut total_columns = 0;
    let mut cell_columns_entry: Option<&MergeableDataObjectEntry> = None;

    for map_entry in &custom_map.map_entry {
        let key_index = map_entry.key as usize;
        if key_index == 0 || key_index > key_items.len() {
            continue;
        }

        let key_name = &key_items[key_index - 1];
        let object_index = map_entry.value.object_index as usize;

        if object_index >= table_objects.len() {
            continue;
        }

        let target_object = &table_objects[object_index];

        match key_name.as_str() {
            "crRows" => {
                total_rows = parse_rows(target_object, uuid_items, table_objects, &mut row_indices);
            }
            "crColumns" => {
                total_columns = parse_columns(
                    target_object,
                    uuid_items,
                    table_objects,
                    &mut column_indices,
                );
            }
            "cellColumns" => {
                cell_columns_entry = Some(target_object);
            }
            _ => {}
        }
    }

    if total_rows == 0 || total_columns == 0 {
        return None;
    }

    let mut reconstructed_table = vec![vec![String::new(); total_columns]; total_rows];

    if let Some(cell_columns) = cell_columns_entry {
        parse_cell_columns(
            cell_columns,
            table_objects,
            &row_indices,
            &column_indices,
            &mut reconstructed_table,
        );
    }

    if table_direction == RIGHT_TO_LEFT_DIRECTION {
        for row in &mut reconstructed_table {
            row.reverse();
        }
    }

    Some(Table {
        rows: reconstructed_table,
        direction: table_direction.to_string(),
    })
}

fn parse_rows(
    object_entry: &MergeableDataObjectEntry,
    uuid_items: &[Vec<u8>],
    table_objects: &[MergeableDataObjectEntry],
    row_indices: &mut HashMap<u64, usize>,
) -> usize {
    let ordered_set = match &object_entry.ordered_set {
        Some(set) => set,
        None => return 0,
    };

    let mut total_rows = 0;

    for attachment in &ordered_set.ordering.array.attachment {
        if let Some(uuid_index) = uuid_items.iter().position(|u| u == &attachment.uuid) {
            row_indices.insert(uuid_index as u64, total_rows);
            total_rows += 1;
        }
    }

    for element in &ordered_set.ordering.contents.element {
        let key_index = element.key.object_index as usize;
        let value_index = element.value.object_index as usize;

        if key_index >= table_objects.len() || value_index >= table_objects.len() {
            continue;
        }

        if let (Some(key_uuid), Some(value_uuid)) = (
            get_target_uuid_from_object_entry(&table_objects[key_index]),
            get_target_uuid_from_object_entry(&table_objects[value_index]),
        ) && let Some(&key_row_index) = row_indices.get(&key_uuid)
        {
            row_indices.insert(value_uuid, key_row_index);
        }
    }

    total_rows
}

fn parse_columns(
    object_entry: &MergeableDataObjectEntry,
    uuid_items: &[Vec<u8>],
    table_objects: &[MergeableDataObjectEntry],
    column_indices: &mut HashMap<u64, usize>,
) -> usize {
    let ordered_set = match &object_entry.ordered_set {
        Some(set) => set,
        None => return 0,
    };

    let mut total_columns = 0;

    for attachment in &ordered_set.ordering.array.attachment {
        if let Some(uuid_index) = uuid_items.iter().position(|u| u == &attachment.uuid) {
            column_indices.insert(uuid_index as u64, total_columns);
            total_columns += 1;
        }
    }

    for element in &ordered_set.ordering.contents.element {
        let key_index = element.key.object_index as usize;
        let value_index = element.value.object_index as usize;

        if key_index >= table_objects.len() || value_index >= table_objects.len() {
            continue;
        }

        if let (Some(key_uuid), Some(value_uuid)) = (
            get_target_uuid_from_object_entry(&table_objects[key_index]),
            get_target_uuid_from_object_entry(&table_objects[value_index]),
        ) && let Some(&key_col_index) = column_indices.get(&key_uuid)
        {
            column_indices.insert(value_uuid, key_col_index);
        }
    }

    total_columns
}

fn parse_cell_columns(
    cell_columns_entry: &MergeableDataObjectEntry,
    table_objects: &[MergeableDataObjectEntry],
    row_indices: &HashMap<u64, usize>,
    column_indices: &HashMap<u64, usize>,
    reconstructed_table: &mut [Vec<String>],
) {
    let dictionary = match &cell_columns_entry.dictionary {
        Some(dict) => dict,
        None => return,
    };

    for column in &dictionary.element {
        let column_index = column.key.object_index as usize;
        if column_index >= table_objects.len() {
            continue;
        }

        let current_column = match get_target_uuid_from_object_entry(&table_objects[column_index]) {
            Some(uuid) => uuid,
            None => continue,
        };

        let target_dict_index = column.value.object_index as usize;
        if target_dict_index >= table_objects.len() {
            continue;
        }

        let target_dictionary_object = &table_objects[target_dict_index];
        let target_dict = match &target_dictionary_object.dictionary {
            Some(dict) => dict,
            None => continue,
        };

        for row in &target_dict.element {
            let row_index = row.key.object_index as usize;
            if row_index >= table_objects.len() {
                continue;
            }

            let current_row = match get_target_uuid_from_object_entry(&table_objects[row_index]) {
                Some(uuid) => uuid,
                None => continue,
            };

            let target_cell_index = row.value.object_index as usize;
            if target_cell_index >= table_objects.len() {
                continue;
            }

            let target_cell = &table_objects[target_cell_index];

            let cell_text = if let Some(ref note) = target_cell.note {
                note.note_text.clone()
            } else {
                String::new()
            };

            if let (Some(&row_idx), Some(&col_idx)) = (
                row_indices.get(&current_row),
                column_indices.get(&current_column),
            ) && row_idx < reconstructed_table.len()
                && col_idx < reconstructed_table[row_idx].len()
            {
                reconstructed_table[row_idx][col_idx] = cell_text;
            }
        }
    }
}
