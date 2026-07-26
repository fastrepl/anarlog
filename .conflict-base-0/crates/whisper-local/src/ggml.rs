#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct GgmlBackend {
    pub kind: String,
    pub name: String,
    pub description: String,
    pub total_memory_mb: usize,
    pub free_memory_mb: usize,
}

#[cfg(not(all(feature = "actual", not(debug_assertions))))]
pub fn list_ggml_backends() -> Vec<GgmlBackend> {
    vec![]
}

// https://github.com/ggml-org/llama.cpp/blob/3a9457d/common/arg.cpp#L2300
#[cfg(all(feature = "actual", not(debug_assertions)))]
pub fn list_ggml_backends() -> Vec<GgmlBackend> {
    use whisper_rs::whisper_rs_sys::{
        ggml_backend_dev_count, ggml_backend_dev_description, ggml_backend_dev_get,
        ggml_backend_dev_memory, ggml_backend_dev_name, ggml_backend_dev_type,
        ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_ACCEL as GGML_BACKEND_DEVICE_TYPE_ACCEL,
        ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_CPU as GGML_BACKEND_DEVICE_TYPE_CPU,
        ggml_backend_dev_type_GGML_BACKEND_DEVICE_TYPE_GPU as GGML_BACKEND_DEVICE_TYPE_GPU,
    };

    let count = unsafe { ggml_backend_dev_count() };
    let mut devices = Vec::with_capacity(count);

    for i in 0..count {
        unsafe {
            let dev = ggml_backend_dev_get(i);

            let kind: String = match ggml_backend_dev_type(dev) {
                GGML_BACKEND_DEVICE_TYPE_CPU => "CPU".into(),
                GGML_BACKEND_DEVICE_TYPE_ACCEL => "ACCEL".into(),
                GGML_BACKEND_DEVICE_TYPE_GPU => "GPU".into(),
                _ => "UNKNOWN".into(),
            };

            let name = std::ffi::CStr::from_ptr(ggml_backend_dev_name(dev))
                .to_string_lossy()
                .into_owned();
            let description = std::ffi::CStr::from_ptr(ggml_backend_dev_description(dev))
                .to_string_lossy()
                .into_owned();

            let mut free_mem: usize = 0;
            let mut total_mem: usize = 0;
            ggml_backend_dev_memory(dev, &mut free_mem, &mut total_mem);

            devices.push(GgmlBackend {
                kind,
                name,
                description,
                total_memory_mb: total_mem / 1024 / 1024,
                free_memory_mb: free_mem / 1024 / 1024,
            });
        }
    }

    devices
}
