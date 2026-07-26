use aws_sdk_bedrock::types::{InferenceType, ModelCustomization, ModelModality};

use crate::Result;
use crate::commands::{
    FoundationModelSummary, ListFoundationModelsRequest, ListFoundationModelsResponse,
};

pub struct Bedrock<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Bedrock<'a, R, M> {
    pub async fn list_foundation_models(
        &self,
        request: ListFoundationModelsRequest,
    ) -> Result<ListFoundationModelsResponse> {
        let state = self.manager.state::<crate::ManagedState>();
        let client = state.client().await;

        let mut builder = client.list_foundation_models();

        if let Some(provider) = &request.by_provider {
            builder = builder.set_by_provider(Some(provider.clone()));
        }

        if let Some(customization_type) = &request.by_customization_type {
            let ct = customization_type.parse::<ModelCustomization>().unwrap();
            builder = builder.set_by_customization_type(Some(ct));
        }

        if let Some(output_modality) = &request.by_output_modality {
            let om = output_modality.parse::<ModelModality>().unwrap();
            builder = builder.set_by_output_modality(Some(om));
        }

        if let Some(inference_type) = &request.by_inference_type {
            let it = inference_type.parse::<InferenceType>().unwrap();
            builder = builder.set_by_inference_type(Some(it));
        }

        let output = builder
            .send()
            .await
            .map_err(|e| crate::Error::AwsSdk(e.to_string()))?;

        let model_summaries: Vec<FoundationModelSummary> = output
            .model_summaries()
            .iter()
            .map(|model| {
                let input_mods = Some(
                    model
                        .input_modalities()
                        .iter()
                        .map(|m| m.as_str().to_string())
                        .collect(),
                );

                let output_mods = Some(
                    model
                        .output_modalities()
                        .iter()
                        .map(|m| m.as_str().to_string())
                        .collect(),
                );

                let customizations = Some(
                    model
                        .customizations_supported()
                        .iter()
                        .map(|c| c.as_str().to_string())
                        .collect(),
                );

                let inference_types = Some(
                    model
                        .inference_types_supported()
                        .iter()
                        .map(|t| t.as_str().to_string())
                        .collect(),
                );

                FoundationModelSummary {
                    model_id: Some(model.model_id().to_string()),
                    model_name: model.model_name().map(|s: &str| s.to_string()),
                    provider_name: model.provider_name().map(|s: &str| s.to_string()),
                    input_modalities: input_mods,
                    output_modalities: output_mods,
                    response_streaming_supported: model.response_streaming_supported(),
                    customizations_supported: customizations,
                    inference_types_supported: inference_types,
                    model_arn: Some(model.model_arn().to_string()),
                }
            })
            .collect();

        Ok(ListFoundationModelsResponse { model_summaries })
    }
}

pub trait BedrockPluginExt<R: tauri::Runtime> {
    fn bedrock(&self) -> Bedrock<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> BedrockPluginExt<R> for T {
    fn bedrock(&self) -> Bedrock<'_, R, Self>
    where
        Self: Sized,
    {
        Bedrock {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}
