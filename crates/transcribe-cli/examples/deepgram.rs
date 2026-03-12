transcribe_cli::simple_provider_example! {
    adapter: owhisper_client::DeepgramAdapter,
    api_base: "https://api.deepgram.com/v1",
    api_key_env: "DEEPGRAM_API_KEY",
    params: {
        let mut params = transcribe_cli::default_listen_params();
        params.model = Some("nova-3".to_string());
        params
    },
}
