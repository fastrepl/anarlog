# Modifications de ce fork

Ce document recense tout ce qui diverge de `fastrepl/anarlog`, pour pouvoir
réappliquer ou abandonner chaque changement après une mise à jour upstream.

Chaque modification dans le code est marquée par un commentaire `[fork]` afin
d'être retrouvée par recherche :

```bash
grep -rn "\[fork\]" --include="*.rs" --include="*.tsx" --include="*.ts" .
```

## 1. Compatibilité macOS 27 / SDK 27

Sans ces trois correctifs, le dépôt **ne compile pas** sur un Mac équipé du
SDK 27. Ce sont des corrections de compatibilité, pas des choix : elles
devraient disparaître quand upstream mettra ses dépendances à jour, et
méritent d'être proposées en amont.

| Fichier | Changement | Raison |
|---|---|---|
| `Cargo.toml` | `swift-rs` → `fd82965` (1.0.8) | les versions antérieures ignorent le nouveau layout de sortie de SwiftPM (`out/Products/Debug`) |
| `rust-toolchain.toml` | `1.94.0` → `stable` | rustc 1.94 rend les proc-macros inchargeables dès que `MACOSX_DEPLOYMENT_TARGET` est défini |
| `plugins/permissions/swift/check-permissions.swift` | `+ import ApplicationServices` | `AXIsProcessTrusted` n'est plus exposé par import transitif |

## 2. Vie privée

Aucune donnée d'usage ni rapport de crash ne quitte la machine.

| Fichier | Changement |
|---|---|
| `plugins/analytics/src/lib.rs` | client analytics construit sans backend PostHog |
| `plugins/analytics/src/ext.rs` | `APP_VERSION` devient optionnel (repli sur la version du package) |
| `apps/desktop/src-tauri/src/lib.rs` | Sentry retiré (plus de client, plus de plugin) |
| `apps/desktop/src/settings/privacy/index.tsx` | interrupteurs PostHog et rapports d'erreur retirés (ils ne pilotaient plus rien) |
| `plugins/updater2/src/ext.rs` | `Updater2::check()` renvoie toujours `Ok(None)` |
| `plugins/updater2/src/lib.rs` | boucle native de vérification/installation auto (30 min) retirée |

Note : Sentry était déjà inactif sans `SENTRY_DSN` au build (`option_env!`).
Le retrait explicite sert surtout à supprimer la dépendance du build.

Note sur l'updater : `plugins.updater.active: false` dans `tauri.conf.stable.json`
(voir historique git) ne coupe que la génération d'artefacts de mise à jour au
build — pas les appels runtime. Sans le correctif ci-dessus, l'app interrogeait
quand même `desktop.anarlog.so` (le serveur officiel upstream) et pouvait
**installer automatiquement le binaire officiel par-dessus ce fork**, effaçant
au passage tous les patchs de vie privée et de déblocage Pro. `Updater2::check()`
est le point de passage unique de tout le plugin (vérification manuelle,
téléchargement, boucle native) : le couper là suffit, sans toucher au frontend
(`apps/desktop/src/main/update-banner.tsx`) ni à ses tests.

## 3. Déverrouillage des fonctions Pro

Une seule ligne, volontairement : `isPro: true` dans le contexte de
facturation (`apps/desktop/src/auth/billing.tsx`). Tout le reste du fichier est
identique à upstream.

Cela déverrouille l'interface. Les fonctions **locales** (dictée, dictionnaire,
transcription, intelligence) deviennent réellement utilisables ; celles qui
dépendent d'un serveur (sync, partage, IA hébergée) restent inopérantes — leur
disponibilité se décide côté serveur.

Les écrans Billing, Teams et Sync sont retirés du menu et du routage, mais
**leurs fichiers sont conservés** : modifier deux fichiers crée moins de
conflits que supprimer huit.

## 4. Allègement du build

Plugins retirés de `apps/desktop/src-tauri/Cargo.toml`, de `src/lib.rs` et de
`capabilities/default.json` :

| Plugin | Gain | Statut upstream |
|---|---|---|
| `git` | ~50 crates (`gix`) | déclaré mais **jamais monté** — code mort |
| `screen` | ~17 dépendances | déclaré mais **jamais monté** — code mort |
| `bedrock` | ~27 crates (`aws-*`) | actif ; inutile hors AWS Bedrock |
| `sentry` | ~11 crates | actif ; voir section 2 |
| `updater` (officiel Tauri) | — | actif ; inutile sur un fork |

**⚠️ `updater2` doit rester monté.** Son retrait fait paniquer `tauri-specta`
au démarrage (`Event update-available-event not found in registry!`) : le
plugin expose un événement que le reste de l'application attend. L'app se
termine alors avant d'ouvrir sa fenêtre.

La barre de devtools est également retirée (`apps/desktop/src/main/shell-frame.tsx`),
elle était activée en dur par le mode debug.

## 5. Après une mise à jour upstream

```bash
git fetch upstream && git rebase upstream/main
```

Ordre de résolution conseillé :

1. **Section 1** — vérifier si upstream a relevé `swift-rs` et sa toolchain ;
   si oui, abandonner nos correctifs.
2. **Sections 2 à 4** — réappliquer ; les commentaires `[fork]` marquent les
   points d'insertion.
3. Reconstruire et **vérifier que la fenêtre s'ouvre** : une panique de
   `tauri-specta` tue l'application silencieusement, sans message visible
   ailleurs que sur stderr.

## Pièges rencontrés

- Le `target/debug` atteint **35 Go** après quelques cycles de diagnostic.
  Un `cargo clean` périodique évite de saturer le disque en pleine compilation.
- Les erreurs de démarrage n'apparaissent pas dans `~/Library/Logs/com.hyprnote.dev/app.log`
  (écriture asynchrone, perdue si le processus meurt). Il faut lire **stderr**.
- Une app dev n'est pas un bundle `.app` : les permissions macOS d'audio
  système, d'accessibilité et de calendrier ne peuvent pas être accordées,
  faute d'apparaître dans les Réglages Système.
