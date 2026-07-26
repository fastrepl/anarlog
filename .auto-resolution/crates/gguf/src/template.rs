#[derive(Debug)]
pub enum ChatTemplate {
    TemplateKey(LlamaCppRegistry),
    TemplateValue(String),
}

// https://github.com/ggml-org/llama.cpp/blob/8a8c4ceb6050bd9392609114ca56ae6d26f5b8f5/src/llama-chat.cpp#L27-L61
#[derive(Debug, strum::Display, strum::AsRefStr)]
pub enum LlamaCppRegistry {
    #[strum(serialize = "chatml")]
    ChatML,
    #[strum(serialize = "llama2")]
    Llama2,
    #[strum(serialize = "llama2-sys")]
    Llama2Sys,
    #[strum(serialize = "llama2-sys-bos")]
    Llama2SysBos,
    #[strum(serialize = "llama2-sys-strip")]
    Llama2SysStrip,
    #[strum(serialize = "mistral-v1")]
    MistralV1,
    #[strum(serialize = "mistral-v3")]
    MistralV3,
    #[strum(serialize = "mistral-v3-tekken")]
    MistralV3Tekken,
    #[strum(serialize = "mistral-v7")]
    MistralV7,
    #[strum(serialize = "phi3")]
    Phi3,
    #[strum(serialize = "phi4")]
    Phi4,
    #[strum(serialize = "falcon3")]
    Falcon3,
    #[strum(serialize = "zephyr")]
    Zephyr,
    #[strum(serialize = "monarch")]
    Monarch,
    #[strum(serialize = "gemma")]
    Gemma,
    #[strum(serialize = "orion")]
    Orion,
    #[strum(serialize = "openchat")]
    Openchat,
    #[strum(serialize = "vicuna")]
    Vicuna,
    #[strum(serialize = "vicuna-orca")]
    VicunaOrca,
    #[strum(serialize = "deepseek")]
    Deepseek,
    #[strum(serialize = "deepseek2")]
    Deepseek2,
    #[strum(serialize = "deepseek3")]
    Deepseek3,
    #[strum(serialize = "command-r")]
    CommandR,
    #[strum(serialize = "llama3")]
    Llama3,
    #[strum(serialize = "chatglm3")]
    Chatglm3,
    #[strum(serialize = "chatglm4")]
    Chatglm4,
    #[strum(serialize = "glmedge")]
    Glmedge,
    #[strum(serialize = "minicpm")]
    Minicpm,
    #[strum(serialize = "exaone3")]
    Exaone3,
    #[strum(serialize = "rwkv-world")]
    RwkvWorld,
    #[strum(serialize = "granite")]
    Granite,
    #[strum(serialize = "gigachat")]
    Gigachat,
    #[strum(serialize = "megrez")]
    Megrez,
}

impl AsRef<str> for ChatTemplate {
    fn as_ref(&self) -> &str {
        match self {
            Self::TemplateKey(k) => k.as_ref(),
            Self::TemplateValue(v) => v.as_ref(),
        }
    }
}
