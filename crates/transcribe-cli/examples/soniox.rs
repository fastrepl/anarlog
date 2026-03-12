transcribe_cli::simple_provider_example! {
    adapter: owhisper_client::SonioxAdapter,
    api_base: "https://api.soniox.com",
    api_key_env: "SONIOX_API_KEY",
    params: transcribe_cli::default_listen_params(),
}
