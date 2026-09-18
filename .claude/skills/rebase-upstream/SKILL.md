---
name: rebase-upstream
description: Met à jour ce fork depuis fastrepl/anarlog en préservant ses modifications. À utiliser quand l'utilisateur veut récupérer les changements upstream, rebaser, merger upstream/main, ou demande pourquoi l'app ne démarre plus après une mise à jour.
---

# Rebaser ce fork sur upstream

Ce fork diverge de `fastrepl/anarlog` sur quatre axes, décrits dans `FORK.md`.
Les points d'insertion dans le code portent le marqueur `[fork]` :

```bash
grep -rn "\[fork\]" --include="*.rs" --include="*.tsx" --include="*.ts" . | grep -v target
```

## Avant de commencer

1. **Vérifier l'espace disque.** Un rebase suivi d'une recompilation demande
   au moins 15 Go. Le `target/debug` atteint 35 Go après quelques cycles.

   ```bash
   df -h / && du -sh apps/desktop/src-tauri/target 2>/dev/null
   ```

   Si nécessaire : `cargo clean` dans `apps/desktop/src-tauri`.

2. **Vérifier que tout est commité.** Le rebase écrasera le travail en cours.

## Rebase

```bash
git fetch upstream
git rebase upstream/main
```

Les commits du fork sont thématiques, ce qui permet de les traiter séparément :

| Commit | Traitement |
|---|---|
| `build: compatibilité macOS SDK 27` | **Vérifier s'il est encore nécessaire.** Si upstream a relevé `swift-rs` au-delà de 1.0.8 et sa toolchain au-delà de 1.94, abandonner ce commit (`git rebase --skip`). |
| `privacy: retirer la télémétrie` | Réappliquer. Chercher `[fork]` dans `plugins/analytics/`. |
| `build: retirer les plugins inutilisés` | Réappliquer, **sauf si upstream monte de nouveaux plugins** (voir piège ci-dessous). |
| `feat: déverrouiller les fonctions Pro` | Réappliquer. Une seule ligne : `isPro: true` dans `apps/desktop/src/auth/billing.tsx`. |
| `chore: retirer le site web et l'app mobile` | Conflits « supprimé par nous / modifié par eux » : `git rm -r apps/web apps/mobile` et continuer. |
| Renommage BlackMushi | La plus grosse surface de conflit. Résoudre en gardant la version renommée. |

## Pièges connus

### Ne jamais retirer `updater2`

Son retrait fait paniquer `tauri-specta` au démarrage :

```
thread 'main' panicked at tauri-specta/src/event.rs:
Event update-available-event not found in registry!
```

L'application se termine **avant d'ouvrir sa fenêtre**. Le processus reste
visible, figé à 0 % de CPU dans le shutdown du runtime Tokio.

Règle générale : un plugin peut exposer des événements attendus ailleurs,
même s'il n'est jamais appelé directement. Après avoir retiré un plugin,
toujours lancer l'app et vérifier que la fenêtre s'ouvre.

### Les erreurs de démarrage ne sont pas dans le log applicatif

`~/Library/Logs/com.hyprnote.dev/app.log` s'écrit en asynchrone : si le
processus meurt, les dernières lignes sont perdues. Les paniques de démarrage
n'apparaissent que sur **stderr** :

```bash
pnpm exec turbo dev:desktop > /tmp/dev.log 2>&1
grep -iE "panicked|not found in registry" /tmp/dev.log
```

Ne jamais diagnostiquer un démarrage avec un pipe (`| tail`) : la sortie est
retenue jusqu'à la fin de la commande.

### Vérifier le bon processus

`pgrep -f "target/debug/desktop"` matche aussi le runner node qui le lance.
Utiliser le nom exact :

```bash
pgrep -x desktop
```

Et vérifier que le processus tourne réellement, plutôt que d'être figé en
sortie :

```bash
sample $(pgrep -x desktop) 2 -file /tmp/s.txt
grep -q "drop_glue.*Runtime" /tmp/s.txt && echo "FIGÉ" || echo "VIVANT"
```

Un processus figé consomme ~119 Mo et 0 % de CPU ; un processus sain dépasse
240 Mo.

### Les permissions et capabilities vont par paire

Retirer un plugin de `Cargo.toml` et de `src/lib.rs` ne suffit pas : sa
permission doit aussi disparaître de `apps/desktop/src-tauri/capabilities/default.json`,
sinon Tauri refuse de démarrer sur `Permission <plugin>:default not found`.

## Vérification après rebase

```bash
cd apps/desktop && npx tsc --noEmit
cd src-tauri && cargo check
```

Puis lancer l'application et **confirmer visuellement que la fenêtre s'ouvre**
— un typecheck et un `cargo check` verts ne garantissent pas le démarrage.

Enfin, mettre `FORK.md` à jour si la liste des divergences a changé.
