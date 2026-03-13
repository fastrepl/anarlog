pub mod filters;
mod validate;

pub use filters::{
    TEMPLATE_FILTERS, current_date, is_english, is_korean, language, set_current_date_override,
};
pub use validate::{TemplateUsage, extract};

#[macro_export]
macro_rules! tpl_snapshot {
    ($name:ident, $input:expr, @$($expected:tt)*) => {
        #[test]
        fn $name() {
            insta::assert_snapshot!(askama::Template::render(&$input).unwrap(), @$($expected)*);
        }
    };
    ($name:ident, $input:expr, fixed_date = $date:expr, @$($expected:tt)*) => {
        #[test]
        fn $name() {
            $crate::set_current_date_override(Some($date.to_string()));
            insta::assert_snapshot!(askama::Template::render(&$input).unwrap(), @$($expected)*);
        }
    };
}

#[macro_export]
macro_rules! tpl_assert {
    ($name:ident, $input:expr, $predicate:expr) => {
        #[test]
        fn $name() {
            let rendered: String = askama::Template::render(&$input).unwrap();
            let predicate: fn(&str) -> bool = $predicate;
            assert!(predicate(&rendered), "{}", rendered);
        }
    };
}

#[macro_export]
macro_rules! tpl_snapshot_with_assert {
    ($name:ident, $input:expr, $predicate:expr, @$($expected:tt)*) => {
        #[test]
        fn $name() {
            let rendered: String = askama::Template::render(&$input).unwrap();
            let predicate: fn(&str) -> bool = $predicate;
            assert!(predicate(&rendered), "{}", rendered);
            insta::assert_snapshot!(rendered, @$($expected)*);
        }
    };
    ($name:ident, $input:expr, $predicate:expr, fixed_date = $date:expr, @$($expected:tt)*) => {
        #[test]
        fn $name() {
            $crate::set_current_date_override(Some($date.to_string()));
            let rendered: String = askama::Template::render(&$input).unwrap();
            let predicate: fn(&str) -> bool = $predicate;
            assert!(predicate(&rendered), "{}", rendered);
            insta::assert_snapshot!(rendered, @$($expected)*);
        }
    };
}
