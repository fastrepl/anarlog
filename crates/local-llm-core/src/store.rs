use crate::{Error, ModelSelection, SupportedModel};

pub trait ModelStore: Send + Sync {
    fn get_model(&self) -> Result<Option<SupportedModel>, Error>;
    fn set_model(&self, model: &SupportedModel) -> Result<(), Error>;
    fn get_model_selection(&self) -> Result<Option<ModelSelection>, Error>;
    fn set_model_selection(&self, selection: &ModelSelection) -> Result<(), Error>;
    fn is_default_model_migrated(&self) -> Result<bool, Error>;
    fn set_default_model_migrated(&self, val: bool) -> Result<(), Error>;
}
