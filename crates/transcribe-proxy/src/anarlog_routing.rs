use std::collections::HashSet;

use anlg_language::Language;
use owhisper_client::{AdapterKind, LanguageSupport, Provider};

const DEFAULT_NUM_RETRIES: usize = 2;
const DEFAULT_MAX_DELAY_SECS: u64 = 5;

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub num_retries: usize,
    pub max_delay_secs: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            num_retries: DEFAULT_NUM_RETRIES,
            max_delay_secs: DEFAULT_MAX_DELAY_SECS,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnarlogRoutingConfig {
    pub priorities: Vec<Provider>,
    pub retry_config: RetryConfig,
}

impl Default for AnarlogRoutingConfig {
    fn default() -> Self {
        Self {
            priorities: vec![
                Provider::Soniox,
                Provider::Deepgram,
                Provider::AssemblyAI,
                Provider::Gladia,
                Provider::ElevenLabs,
                Provider::OpenAI,
                Provider::Mistral,
                Provider::DashScope,
            ],
            retry_config: RetryConfig::default(),
        }
    }
}

pub struct AnarlogRouter {
    priorities: Vec<Provider>,
    retry_config: RetryConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingMode {
    Live,
    Batch,
}

impl AnarlogRouter {
    pub fn new(config: AnarlogRoutingConfig) -> Self {
        Self {
            priorities: config.priorities,
            retry_config: config.retry_config,
        }
    }

    pub fn select_provider(
        &self,
        languages: &[Language],
        available_providers: &HashSet<Provider>,
    ) -> Option<Provider> {
        self.select_provider_chain_with_mode(RoutingMode::Live, languages, available_providers)
            .into_iter()
            .next()
    }

    pub fn select_provider_chain(
        &self,
        languages: &[Language],
        available_providers: &HashSet<Provider>,
    ) -> Vec<Provider> {
        self.select_provider_chain_with_mode(RoutingMode::Live, languages, available_providers)
    }

    pub fn select_provider_chain_with_mode(
        &self,
        mode: RoutingMode,
        languages: &[Language],
        available_providers: &HashSet<Provider>,
    ) -> Vec<Provider> {
        let priorities = &self.priorities;
        let mut candidates: Vec<_> = priorities
            .iter()
            .copied()
            .filter_map(|p| {
                let support = self.get_language_support(mode, &p, languages, available_providers);
                if support.is_supported() {
                    Some((p, support))
                } else {
                    None
                }
            })
            .collect();

        candidates.sort_by(|a, b| {
            let (p1, s1) = a;
            let (p2, s2) = b;
            if mode == RoutingMode::Batch || priorities.first() == Some(&Provider::Soniox) {
                match (*p1 == Provider::Soniox, *p2 == Provider::Soniox) {
                    (true, false) => return std::cmp::Ordering::Less,
                    (false, true) => return std::cmp::Ordering::Greater,
                    _ => {}
                }
            }

            match s2.cmp(s1) {
                std::cmp::Ordering::Equal => {
                    let idx1 = priorities
                        .iter()
                        .position(|p| p == p1)
                        .unwrap_or(usize::MAX);
                    let idx2 = priorities
                        .iter()
                        .position(|p| p == p2)
                        .unwrap_or(usize::MAX);
                    idx1.cmp(&idx2)
                }
                other => other,
            }
        });

        candidates.into_iter().map(|(p, _)| p).collect()
    }

    fn get_language_support(
        &self,
        mode: RoutingMode,
        provider: &Provider,
        languages: &[Language],
        available_providers: &HashSet<Provider>,
    ) -> LanguageSupport {
        if !available_providers.contains(provider) {
            return LanguageSupport::NotSupported;
        }
        match mode {
            RoutingMode::Live => {
                AdapterKind::from(*provider).language_support_live(languages, None)
            }
            RoutingMode::Batch => {
                AdapterKind::from(*provider).language_support_batch(languages, None)
            }
        }
    }

    pub fn retry_config(&self) -> &RetryConfig {
        &self.retry_config
    }
}

impl Default for AnarlogRouter {
    fn default() -> Self {
        Self::new(AnarlogRoutingConfig::default())
    }
}

pub fn is_retryable_error(error: &str) -> bool {
    let error_lower = error.to_lowercase();

    let is_auth_error = error_lower.contains("401")
        || error_lower.contains("403")
        || error_lower.contains("unauthorized")
        || error_lower.contains("forbidden");

    let is_client_error = error_lower.contains("400") || error_lower.contains("invalid");

    if is_auth_error || is_client_error {
        return false;
    }

    error_lower.contains("timeout")
        || error_lower.contains("connection")
        || error_lower.contains("500")
        || error_lower.contains("502")
        || error_lower.contains("503")
        || error_lower.contains("504")
        || error_lower.contains("temporarily")
        || error_lower.contains("rate limit")
        || error_lower.contains("too many requests")
}

pub fn should_use_anarlog_routing(provider_param: Option<&str>) -> bool {
    matches!(provider_param, Some("anarlog" | "hyprnote"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_language::ISO639;

    fn langs(codes: &[ISO639]) -> Vec<Language> {
        codes.iter().map(|&c| Language::new(c)).collect()
    }

    fn default_available() -> HashSet<Provider> {
        [Provider::Deepgram, Provider::Soniox].into_iter().collect()
    }

    fn format_chain(chain: &[Provider]) -> String {
        let names: Vec<_> = chain.iter().map(|p| format!("{p:?}")).collect();
        format!("[{}]", names.join(", "))
    }

    #[test]
    fn select_provider_none_when_no_available() {
        let router = AnarlogRouter::default();
        let selected = router.select_provider(&langs(&[ISO639::En]), &HashSet::new());
        assert_eq!(selected, None);
    }

    #[test]
    fn should_use_anarlog_routing() {
        assert!(super::should_use_anarlog_routing(Some("anarlog")));
        assert!(super::should_use_anarlog_routing(Some("hyprnote")));

        assert!(!super::should_use_anarlog_routing(None));
        assert!(!super::should_use_anarlog_routing(Some("deepgram")));
        assert!(!super::should_use_anarlog_routing(Some("soniox")));
        assert!(!super::should_use_anarlog_routing(Some("assemblyai")));
        assert!(!super::should_use_anarlog_routing(Some("")));
        assert!(!super::should_use_anarlog_routing(Some("auto")));
    }

    const SNAPSHOT_LANGS: &[ISO639] = &[
        ISO639::En,
        ISO639::Es,
        ISO639::Fr,
        ISO639::De,
        ISO639::Hi,
        ISO639::Ru,
        ISO639::Pt,
        ISO639::Ja,
        ISO639::It,
        ISO639::Nl,
        ISO639::Ko,
    ];

    #[test]
    fn routing_table() {
        use itertools::Itertools;

        let router = AnarlogRouter::default();
        let available = default_available();

        let mut table = String::new();
        for size in 1..=3 {
            for combo in SNAPSHOT_LANGS.iter().combinations(size) {
                let codes: Vec<ISO639> = combo.into_iter().copied().collect();
                let chain = router.select_provider_chain(&langs(&codes), &available);
                let label = codes.iter().map(|c| c.code()).collect::<Vec<_>>().join("+");
                table.push_str(&format!("{label:<14} -> {}\n", format_chain(&chain)));
            }
        }

        insta::assert_snapshot!(
            table,
            @r###"
            en             -> [Soniox, Deepgram]
            es             -> [Soniox, Deepgram]
            fr             -> [Soniox, Deepgram]
            de             -> [Soniox, Deepgram]
            hi             -> [Soniox, Deepgram]
            ru             -> [Soniox, Deepgram]
            pt             -> [Soniox, Deepgram]
            ja             -> [Soniox, Deepgram]
            it             -> [Soniox, Deepgram]
            nl             -> [Soniox, Deepgram]
            ko             -> [Soniox, Deepgram]
            en+es          -> [Soniox, Deepgram]
            en+fr          -> [Soniox, Deepgram]
            en+de          -> [Soniox, Deepgram]
            en+hi          -> [Soniox, Deepgram]
            en+ru          -> [Soniox, Deepgram]
            en+pt          -> [Soniox, Deepgram]
            en+ja          -> [Soniox, Deepgram]
            en+it          -> [Soniox, Deepgram]
            en+nl          -> [Soniox, Deepgram]
            en+ko          -> [Soniox]
            es+fr          -> [Soniox, Deepgram]
            es+de          -> [Soniox, Deepgram]
            es+hi          -> [Soniox, Deepgram]
            es+ru          -> [Soniox, Deepgram]
            es+pt          -> [Soniox, Deepgram]
            es+ja          -> [Soniox, Deepgram]
            es+it          -> [Soniox, Deepgram]
            es+nl          -> [Soniox, Deepgram]
            es+ko          -> [Soniox]
            fr+de          -> [Soniox, Deepgram]
            fr+hi          -> [Soniox, Deepgram]
            fr+ru          -> [Soniox, Deepgram]
            fr+pt          -> [Soniox, Deepgram]
            fr+ja          -> [Soniox, Deepgram]
            fr+it          -> [Soniox, Deepgram]
            fr+nl          -> [Soniox, Deepgram]
            fr+ko          -> [Soniox]
            de+hi          -> [Soniox, Deepgram]
            de+ru          -> [Soniox, Deepgram]
            de+pt          -> [Soniox, Deepgram]
            de+ja          -> [Soniox, Deepgram]
            de+it          -> [Soniox, Deepgram]
            de+nl          -> [Soniox, Deepgram]
            de+ko          -> [Soniox]
            hi+ru          -> [Soniox, Deepgram]
            hi+pt          -> [Soniox, Deepgram]
            hi+ja          -> [Soniox, Deepgram]
            hi+it          -> [Soniox, Deepgram]
            hi+nl          -> [Soniox, Deepgram]
            hi+ko          -> [Soniox]
            ru+pt          -> [Soniox, Deepgram]
            ru+ja          -> [Soniox, Deepgram]
            ru+it          -> [Soniox, Deepgram]
            ru+nl          -> [Soniox, Deepgram]
            ru+ko          -> [Soniox]
            pt+ja          -> [Soniox, Deepgram]
            pt+it          -> [Soniox, Deepgram]
            pt+nl          -> [Soniox, Deepgram]
            pt+ko          -> [Soniox]
            ja+it          -> [Soniox, Deepgram]
            ja+nl          -> [Soniox, Deepgram]
            ja+ko          -> [Soniox]
            it+nl          -> [Soniox, Deepgram]
            it+ko          -> [Soniox]
            nl+ko          -> [Soniox]
            en+es+fr       -> [Soniox, Deepgram]
            en+es+de       -> [Soniox, Deepgram]
            en+es+hi       -> [Soniox, Deepgram]
            en+es+ru       -> [Soniox, Deepgram]
            en+es+pt       -> [Soniox, Deepgram]
            en+es+ja       -> [Soniox, Deepgram]
            en+es+it       -> [Soniox, Deepgram]
            en+es+nl       -> [Soniox, Deepgram]
            en+es+ko       -> [Soniox]
            en+fr+de       -> [Soniox, Deepgram]
            en+fr+hi       -> [Soniox, Deepgram]
            en+fr+ru       -> [Soniox, Deepgram]
            en+fr+pt       -> [Soniox, Deepgram]
            en+fr+ja       -> [Soniox, Deepgram]
            en+fr+it       -> [Soniox, Deepgram]
            en+fr+nl       -> [Soniox, Deepgram]
            en+fr+ko       -> [Soniox]
            en+de+hi       -> [Soniox, Deepgram]
            en+de+ru       -> [Soniox, Deepgram]
            en+de+pt       -> [Soniox, Deepgram]
            en+de+ja       -> [Soniox, Deepgram]
            en+de+it       -> [Soniox, Deepgram]
            en+de+nl       -> [Soniox, Deepgram]
            en+de+ko       -> [Soniox]
            en+hi+ru       -> [Soniox, Deepgram]
            en+hi+pt       -> [Soniox, Deepgram]
            en+hi+ja       -> [Soniox, Deepgram]
            en+hi+it       -> [Soniox, Deepgram]
            en+hi+nl       -> [Soniox, Deepgram]
            en+hi+ko       -> [Soniox]
            en+ru+pt       -> [Soniox, Deepgram]
            en+ru+ja       -> [Soniox, Deepgram]
            en+ru+it       -> [Soniox, Deepgram]
            en+ru+nl       -> [Soniox, Deepgram]
            en+ru+ko       -> [Soniox]
            en+pt+ja       -> [Soniox, Deepgram]
            en+pt+it       -> [Soniox, Deepgram]
            en+pt+nl       -> [Soniox, Deepgram]
            en+pt+ko       -> [Soniox]
            en+ja+it       -> [Soniox, Deepgram]
            en+ja+nl       -> [Soniox, Deepgram]
            en+ja+ko       -> [Soniox]
            en+it+nl       -> [Soniox, Deepgram]
            en+it+ko       -> [Soniox]
            en+nl+ko       -> [Soniox]
            es+fr+de       -> [Soniox, Deepgram]
            es+fr+hi       -> [Soniox, Deepgram]
            es+fr+ru       -> [Soniox, Deepgram]
            es+fr+pt       -> [Soniox, Deepgram]
            es+fr+ja       -> [Soniox, Deepgram]
            es+fr+it       -> [Soniox, Deepgram]
            es+fr+nl       -> [Soniox, Deepgram]
            es+fr+ko       -> [Soniox]
            es+de+hi       -> [Soniox, Deepgram]
            es+de+ru       -> [Soniox, Deepgram]
            es+de+pt       -> [Soniox, Deepgram]
            es+de+ja       -> [Soniox, Deepgram]
            es+de+it       -> [Soniox, Deepgram]
            es+de+nl       -> [Soniox, Deepgram]
            es+de+ko       -> [Soniox]
            es+hi+ru       -> [Soniox, Deepgram]
            es+hi+pt       -> [Soniox, Deepgram]
            es+hi+ja       -> [Soniox, Deepgram]
            es+hi+it       -> [Soniox, Deepgram]
            es+hi+nl       -> [Soniox, Deepgram]
            es+hi+ko       -> [Soniox]
            es+ru+pt       -> [Soniox, Deepgram]
            es+ru+ja       -> [Soniox, Deepgram]
            es+ru+it       -> [Soniox, Deepgram]
            es+ru+nl       -> [Soniox, Deepgram]
            es+ru+ko       -> [Soniox]
            es+pt+ja       -> [Soniox, Deepgram]
            es+pt+it       -> [Soniox, Deepgram]
            es+pt+nl       -> [Soniox, Deepgram]
            es+pt+ko       -> [Soniox]
            es+ja+it       -> [Soniox, Deepgram]
            es+ja+nl       -> [Soniox, Deepgram]
            es+ja+ko       -> [Soniox]
            es+it+nl       -> [Soniox, Deepgram]
            es+it+ko       -> [Soniox]
            es+nl+ko       -> [Soniox]
            fr+de+hi       -> [Soniox, Deepgram]
            fr+de+ru       -> [Soniox, Deepgram]
            fr+de+pt       -> [Soniox, Deepgram]
            fr+de+ja       -> [Soniox, Deepgram]
            fr+de+it       -> [Soniox, Deepgram]
            fr+de+nl       -> [Soniox, Deepgram]
            fr+de+ko       -> [Soniox]
            fr+hi+ru       -> [Soniox, Deepgram]
            fr+hi+pt       -> [Soniox, Deepgram]
            fr+hi+ja       -> [Soniox, Deepgram]
            fr+hi+it       -> [Soniox, Deepgram]
            fr+hi+nl       -> [Soniox, Deepgram]
            fr+hi+ko       -> [Soniox]
            fr+ru+pt       -> [Soniox, Deepgram]
            fr+ru+ja       -> [Soniox, Deepgram]
            fr+ru+it       -> [Soniox, Deepgram]
            fr+ru+nl       -> [Soniox, Deepgram]
            fr+ru+ko       -> [Soniox]
            fr+pt+ja       -> [Soniox, Deepgram]
            fr+pt+it       -> [Soniox, Deepgram]
            fr+pt+nl       -> [Soniox, Deepgram]
            fr+pt+ko       -> [Soniox]
            fr+ja+it       -> [Soniox, Deepgram]
            fr+ja+nl       -> [Soniox, Deepgram]
            fr+ja+ko       -> [Soniox]
            fr+it+nl       -> [Soniox, Deepgram]
            fr+it+ko       -> [Soniox]
            fr+nl+ko       -> [Soniox]
            de+hi+ru       -> [Soniox, Deepgram]
            de+hi+pt       -> [Soniox, Deepgram]
            de+hi+ja       -> [Soniox, Deepgram]
            de+hi+it       -> [Soniox, Deepgram]
            de+hi+nl       -> [Soniox, Deepgram]
            de+hi+ko       -> [Soniox]
            de+ru+pt       -> [Soniox, Deepgram]
            de+ru+ja       -> [Soniox, Deepgram]
            de+ru+it       -> [Soniox, Deepgram]
            de+ru+nl       -> [Soniox, Deepgram]
            de+ru+ko       -> [Soniox]
            de+pt+ja       -> [Soniox, Deepgram]
            de+pt+it       -> [Soniox, Deepgram]
            de+pt+nl       -> [Soniox, Deepgram]
            de+pt+ko       -> [Soniox]
            de+ja+it       -> [Soniox, Deepgram]
            de+ja+nl       -> [Soniox, Deepgram]
            de+ja+ko       -> [Soniox]
            de+it+nl       -> [Soniox, Deepgram]
            de+it+ko       -> [Soniox]
            de+nl+ko       -> [Soniox]
            hi+ru+pt       -> [Soniox, Deepgram]
            hi+ru+ja       -> [Soniox, Deepgram]
            hi+ru+it       -> [Soniox, Deepgram]
            hi+ru+nl       -> [Soniox, Deepgram]
            hi+ru+ko       -> [Soniox]
            hi+pt+ja       -> [Soniox, Deepgram]
            hi+pt+it       -> [Soniox, Deepgram]
            hi+pt+nl       -> [Soniox, Deepgram]
            hi+pt+ko       -> [Soniox]
            hi+ja+it       -> [Soniox, Deepgram]
            hi+ja+nl       -> [Soniox, Deepgram]
            hi+ja+ko       -> [Soniox]
            hi+it+nl       -> [Soniox, Deepgram]
            hi+it+ko       -> [Soniox]
            hi+nl+ko       -> [Soniox]
            ru+pt+ja       -> [Soniox, Deepgram]
            ru+pt+it       -> [Soniox, Deepgram]
            ru+pt+nl       -> [Soniox, Deepgram]
            ru+pt+ko       -> [Soniox]
            ru+ja+it       -> [Soniox, Deepgram]
            ru+ja+nl       -> [Soniox, Deepgram]
            ru+ja+ko       -> [Soniox]
            ru+it+nl       -> [Soniox, Deepgram]
            ru+it+ko       -> [Soniox]
            ru+nl+ko       -> [Soniox]
            pt+ja+it       -> [Soniox, Deepgram]
            pt+ja+nl       -> [Soniox, Deepgram]
            pt+ja+ko       -> [Soniox]
            pt+it+nl       -> [Soniox, Deepgram]
            pt+it+ko       -> [Soniox]
            pt+nl+ko       -> [Soniox]
            ja+it+nl       -> [Soniox, Deepgram]
            ja+it+ko       -> [Soniox]
            ja+nl+ko       -> [Soniox]
            it+nl+ko       -> [Soniox]
            "###
        );
    }

    #[derive(Debug, Clone)]
    struct LangCombo(Vec<Language>);

    impl quickcheck::Arbitrary for LangCombo {
        fn arbitrary(g: &mut quickcheck::Gen) -> Self {
            let count = *g.choose(&[1usize, 2, 3]).unwrap();
            let langs = (0..count)
                .map(|_| Language::new(*g.choose(&codes_iso_639::part_1::ALL_CODES).unwrap()))
                .collect();
            LangCombo(langs)
        }
    }

    #[quickcheck_macros::quickcheck]
    fn prop_select_is_first_of_chain(combo: LangCombo) -> bool {
        let router = AnarlogRouter::default();
        let available = default_available();
        router.select_provider(&combo.0, &available)
            == router
                .select_provider_chain(&combo.0, &available)
                .into_iter()
                .next()
    }

    #[quickcheck_macros::quickcheck]
    fn prop_chain_no_duplicates(combo: LangCombo) -> bool {
        let router = AnarlogRouter::default();
        let available = default_available();
        let chain = router.select_provider_chain(&combo.0, &available);
        let unique: HashSet<_> = chain.iter().collect();
        unique.len() == chain.len()
    }

    #[quickcheck_macros::quickcheck]
    fn prop_chain_subset_of_available(combo: LangCombo) -> bool {
        let router = AnarlogRouter::default();
        let available = default_available();
        router
            .select_provider_chain(&combo.0, &available)
            .iter()
            .all(|p| available.contains(p))
    }

    #[quickcheck_macros::quickcheck]
    fn prop_language_order_independent(combo: LangCombo) -> bool {
        let router = AnarlogRouter::default();
        let available = default_available();
        let mut reversed = combo.0.clone();
        reversed.reverse();
        router.select_provider(&combo.0, &available)
            == router.select_provider(&reversed, &available)
    }

    #[quickcheck_macros::quickcheck]
    fn prop_soniox_always_in_chain_when_supported(combo: LangCombo) -> quickcheck::TestResult {
        let router = AnarlogRouter::default();
        let available = default_available();
        let chain = router.select_provider_chain(&combo.0, &available);
        if chain.is_empty() {
            return quickcheck::TestResult::discard();
        }
        quickcheck::TestResult::from_bool(chain.contains(&Provider::Soniox))
    }

    #[test]
    fn batch_routing_uses_batch_language_support() {
        let router = AnarlogRouter::default();
        let languages = langs(&[ISO639::En]);
        let available: HashSet<Provider> = [Provider::OpenAI].into_iter().collect();

        assert_eq!(
            router.select_provider_chain_with_mode(RoutingMode::Live, &languages, &available),
            vec![Provider::OpenAI]
        );
        assert_eq!(
            router.select_provider_chain_with_mode(RoutingMode::Batch, &languages, &available),
            vec![Provider::OpenAI]
        );
    }

    #[test]
    fn pro_routing_prefers_soniox_in_both_modes() {
        let router = AnarlogRouter::default();
        let languages = langs(&[ISO639::En]);
        let available = default_available();

        for mode in [RoutingMode::Live, RoutingMode::Batch] {
            assert_eq!(
                router.select_provider_chain_with_mode(mode, &languages, &available),
                vec![Provider::Soniox, Provider::Deepgram],
                "{mode:?}"
            );
            assert_eq!(
                router.select_provider_chain_with_mode(
                    mode,
                    &languages,
                    &[Provider::Deepgram].into_iter().collect(),
                ),
                vec![Provider::Deepgram],
                "{mode:?}"
            );
        }
    }

    #[test]
    fn custom_live_priority_can_prefer_deepgram() {
        let router = AnarlogRouter::new(AnarlogRoutingConfig {
            priorities: vec![Provider::Deepgram, Provider::Soniox],
            ..Default::default()
        });
        let languages = langs(&[ISO639::En]);
        let available = default_available();

        assert_eq!(
            router.select_provider_chain_with_mode(RoutingMode::Live, &languages, &available),
            vec![Provider::Deepgram, Provider::Soniox]
        );
        assert_eq!(
            router.select_provider_chain_with_mode(RoutingMode::Batch, &languages, &available),
            vec![Provider::Soniox, Provider::Deepgram]
        );
    }
}
