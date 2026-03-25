pub use hypr_model_manager::{
    ModelLoader, ModelManager as GenericModelManager,
    ModelManagerBuilder as GenericModelManagerBuilder,
};

pub type ModelManager = GenericModelManager<hypr_cactus::Model>;
pub type ModelManagerBuilder = GenericModelManagerBuilder<hypr_cactus::Model>;
