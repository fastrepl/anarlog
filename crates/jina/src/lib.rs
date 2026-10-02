mod client;
mod error;
mod reader;
mod search;
mod types;

pub use client::*;
pub use error::*;
pub use reader::*;
pub use search::*;
pub use types::*;

macro_rules! common_derives {
    ($item:item) => {
        #[derive(
            Debug,
            Eq,
            PartialEq,
            Clone,
            serde::Serialize,
            serde::Deserialize,
            specta::Type,
            schemars::JsonSchema,
        )]
        $item
    };
}

pub(crate) use common_derives;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_missing_api_key() {
        let result = JinaClientBuilder::default().build();
        assert!(result.is_err());
    }
}
