use super::*;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;

#[derive(Type, Serialize, Deserialize)]
struct SimpleStruct {
    name: String,
    age: i32,
    active: bool,
}

#[derive(Type, Serialize, Deserialize)]
struct OptionalFields {
    required: String,
    optional: Option<String>,
    optional_num: Option<i32>,
}

#[derive(Type, Serialize, Deserialize)]
struct WithArray {
    tags: Vec<String>,
    scores: Vec<i32>,
}

#[derive(Type, Serialize, Deserialize)]
enum SimpleEnum {
    One,
    Two,
    Three,
}

#[derive(Type, Serialize, Deserialize)]
struct NewtypeWrapper(String);

#[derive(Type, Serialize, Deserialize)]
struct TupleStruct(String, i32);

#[derive(Type, Serialize, Deserialize)]
struct WithMap {
    data: HashMap<String, i32>,
}

#[derive(Type, Serialize, Deserialize)]
struct WithNestedOptional {
    nested: Option<Vec<String>>,
}

#[derive(Type, Serialize, Deserialize)]
#[serde(tag = "type")]
enum InternallyTaggedEnum {
    Variant1 { value: String },
    Variant2 { count: i32 },
}

#[derive(Type, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
enum AdjacentlyTaggedEnum {
    Text(String),
    Number(i32),
}

#[derive(Type, Serialize, Deserialize)]
#[serde(untagged)]
enum UntaggedEnum {
    Str(String),
    Num(i32),
}

#[derive(Type, Serialize, Deserialize)]
enum ExternallyTaggedEnum {
    Unit,
    WithData(String),
    WithStruct { field: i32 },
}

#[derive(Type, Serialize, Deserialize)]
struct WithReference {
    simple: SimpleStruct,
}

#[derive(Type, Serialize, Deserialize)]
struct WithTuple {
    pair: (String, i32),
}

#[derive(Type, Serialize, Deserialize)]
struct AllPrimitives {
    a_i8: i8,
    a_i16: i16,
    a_i32: i32,
    a_i64: i64,
    a_u8: u8,
    a_u16: u16,
    a_u32: u32,
    a_u64: u64,
    a_f32: f32,
    a_f64: f64,
    a_bool: bool,
    a_string: String,
    a_char: char,
}

#[derive(Type, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
enum FlattenInner {
    A,
    B,
}

#[derive(Type, Serialize, Deserialize)]
struct WithFlatten {
    name: String,
    #[serde(flatten)]
    inner: FlattenInner,
}

#[test]
fn test_simple_struct() {
    let mut types = specta::TypeCollection::default();
    types.register::<SimpleStruct>();
    let output = Zod::new().header("").export(&types).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_optional_fields() {
    let mut types = specta::TypeCollection::default();
    types.register::<OptionalFields>();
    let output = Zod::new().header("").export(&types).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_array_fields() {
    let mut types = specta::TypeCollection::default();
    types.register::<WithArray>();
    let output = Zod::new().header("").export(&types).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_simple_enum() {
    let mut types = specta::TypeCollection::default();
    types.register::<SimpleEnum>();
    let output = Zod::new().header("").export(&types).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_newtype_wrapper() {
    let mut types = specta::TypeCollection::default();
    types.register::<NewtypeWrapper>();
    let output = Zod::new().header("").export(&types).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_tuple_struct() {
    let mut types = specta::TypeCollection::default();
    types.register::<TupleStruct>();
    let output = Zod::new().header("").export(&types).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_map_field() {
    let mut types = specta::TypeCollection::default();
    types.register::<WithMap>();
    let output = Zod::new().header("").export(&types).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_nested_optional() {
    let mut types = specta::TypeCollection::default();
    types.register::<WithNestedOptional>();
    let output = Zod::new().header("").export(&types).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_internally_tagged_enum() {
    let mut types = specta::TypeCollection::default();
    types.register::<InternallyTaggedEnum>();
    let output = Zod::new().header("").export(&types).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_adjacently_tagged_enum() {
    let mut types = specta::TypeCollection::default();
    types.register::<AdjacentlyTaggedEnum>();
    let output = Zod::new().header("").export(&types).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_untagged_enum() {
    let mut types = specta::TypeCollection::default();
    types.register::<UntaggedEnum>();
    let output = Zod::new().header("").export(&types).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_externally_tagged_enum() {
    let mut types = specta::TypeCollection::default();
    types.register::<ExternallyTaggedEnum>();
    let output = Zod::new().header("").export(&types).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_with_reference() {
    let mut types = specta::TypeCollection::default();
    types.register::<WithReference>();
    let output = Zod::new().header("").export(&types).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_tuple_field() {
    let mut types = specta::TypeCollection::default();
    types.register::<WithTuple>();
    let output = Zod::new().header("").export(&types).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_all_primitives() {
    let mut types = specta::TypeCollection::default();
    types.register::<AllPrimitives>();
    let output = Zod::new().header("").export(&types).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_camel_case_conversion() {
    assert_eq!(utils::to_camel_case("SimpleStruct"), "simpleStruct");
    assert_eq!(utils::to_camel_case("my_struct"), "myStruct");
    assert_eq!(utils::to_camel_case("my-struct"), "myStruct");
    assert_eq!(utils::to_camel_case("MyStruct"), "myStruct");
    assert_eq!(utils::to_camel_case("ABC"), "aBC");
}

#[test]
fn test_custom_header() {
    let mut types = specta::TypeCollection::default();
    types.register::<SimpleStruct>();
    let custom_header = "// Custom header\nimport { z } from 'zod';\n\n";
    let output = Zod::new().header(custom_header).export(&types).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_type_export() {
    let mut types = specta::TypeCollection::default();
    types.register::<SimpleStruct>();
    let output = Zod::new().header("").export(&types).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_bigint_string() {
    #[derive(Type, Serialize, Deserialize)]
    struct WithBigInt {
        big: i64,
    }
    let mut types = specta::TypeCollection::default();
    types.register::<WithBigInt>();
    let output = Zod::new()
        .header("")
        .bigint(BigIntExportBehavior::String)
        .export(&types)
        .unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_bigint_bigint() {
    #[derive(Type, Serialize, Deserialize)]
    struct WithBigInt {
        big: u64,
    }
    let mut types = specta::TypeCollection::default();
    types.register::<WithBigInt>();
    let output = Zod::new()
        .header("")
        .bigint(BigIntExportBehavior::BigInt)
        .export(&types)
        .unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_bigint_fail() {
    #[derive(Type, Serialize, Deserialize)]
    struct WithBigInt {
        big: i64,
    }
    let mut types = specta::TypeCollection::default();
    types.register::<WithBigInt>();
    let result = Zod::new()
        .header("")
        .bigint(BigIntExportBehavior::Fail)
        .export(&types);
    assert!(result.is_err());
}

#[test]
fn test_flatten() {
    let mut types = specta::TypeCollection::default();
    types.register::<WithFlatten>();
    let output = Zod::new().header("").export(&types).unwrap();
    insta::assert_snapshot!(output);
}

#[test]
fn test_sanitise_key() {
    assert_eq!(utils::sanitise_key("normal"), "normal");
    assert_eq!(utils::sanitise_key("with_underscore"), "with_underscore");
    assert_eq!(utils::sanitise_key("123start"), "\"123start\"");
    assert_eq!(utils::sanitise_key("has space"), "\"has space\"");
    assert_eq!(utils::sanitise_key("@odata.context"), "\"@odata.context\"");
}

#[test]
fn test_escape_string() {
    assert_eq!(utils::escape_string("normal").as_ref(), "normal");
    assert_eq!(utils::escape_string("has\"quote").as_ref(), "has\\\"quote");
    assert_eq!(utils::escape_string("has\\slash").as_ref(), "has\\\\slash");
    assert_eq!(
        utils::escape_string("has\nnewline").as_ref(),
        "has\\nnewline"
    );
}
