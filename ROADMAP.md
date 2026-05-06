# mdreader — feuille de route

Inspirée d'une comparaison avec [Glow](https://github.com/charmbracelet/glow) (Go).
Les références de fichiers pointent vers `~/Projects/glow` quand applicable.

## Gros morceaux structurels

### 1. Stash mode + file browser
- **Source Glow** : `ui/stash.go`, `ui/stashitem.go`
- Lancement sans argument → scanne récursivement le dossier courant pour trouver tous les `.md`, affiche une liste navigable avec fuzzy-search (`/`), tri par date, pagination.
- C'est ce qui fait que Glow se ressent comme un outil et pas juste un viewer.
- Composants : state machine (mode stash vs pager), recherche fuzzy (crate `nucleo` ou `fuzzy-matcher`), scan de dossier (`walkdir`).

### 2. Sources distantes
- **Source Glow** : `github.go`, `gitlab.go`, `url.go`
- `mdreader owner/repo`, `mdreader github://...`, `mdreader https://...` → résolution via API GitHub/GitLab pour récupérer le README.
- Composants : HTTP client (`reqwest` async ou `ureq` blocking), parsing JSON (`serde_json`), résolution d'URL.

### 3. Auto-reload sur changement de fichier
- **Source Glow** : `ui/pager.go:482-531` (fsnotify)
- Surveille le fichier courant et recharge automatiquement à chaque modification sur disque.
- Composant : crate `notify` (channels + watcher).

## Quick wins (≈ 1 journée chacun)

- [ ] **Raccourcis clavier manquants** (`ui/pager.go:189-244`) : `u/d` (demi-page), `b/f` (page), `e` (edit externe), `c` (copy), `r` (reload), `?` (toggle help). Squelette déjà en place dans `on_key`.
- [ ] **Détection auto de stdin** (`main.go:213-222`) : `cat README.md | mdreader` sans argument. Test via `IsTerminal` (`std::io::IsTerminal`).
- [ ] **Mode pager** (`main.go:318-334`) : flag `-p` qui délègue à `$PAGER` / `less -r` au lieu du TUI.
- [ ] **Frontmatter YAML strip** (`utils/utils.go`) : virer les blocs `---...---` en début de fichier avant parsing.
- [ ] **Barre de statut avec % scroll** (`ui/pager.go:296-366`) : `self.scroll` + `total_lines` déjà dispo, pur cosmétique.
- [ ] **Edit externe** (`ui/editor.go`) : touche `e` → lance `$EDITOR` sur le fichier, recharge en sortie.

## Configuration & thèmes (effort moyen)

- [ ] **Fichier de config** (`config_cmd.go`, viper côté Glow) : TOML dans `$XDG_CONFIG_HOME/mdreader/config.toml` avec prefs (thème, width, mouse, etc.). Crates : `toml` + `serde` + `dirs`.
- [ ] **Thèmes syntect multiples** : aujourd'hui `base16-ocean.dark` en dur dans `render.rs:26`. Ajouter flag `--theme` et exposer ceux de `ThemeSet::load_defaults()` : `InspiredGitHub`, `Solarized (dark)`, `Solarized (light)`, `base16-eighties.dark`, `base16-mocha.dark`, `base16-ocean.light`. Auto-détection clair/sombre via `termbg` plus tard.

## Hors scope (apprentissage Rust)

- ~~Stash serveur / Charm Cloud~~ — infra propriétaire Charmbracelet.
- ~~Commande `man`~~ — utile pour un outil distribué, pas pour apprendre Rust.

## Ordre de parcours suggéré

1. **Quick wins clavier + barre de statut** — consolide ratatui sans nouveaux crates.
2. **Frontmatter + stdin auto + `--theme`** — petites manipulations `String`/`io`/args. Décider : `clap` ou parsing manuel.
3. **Auto-reload avec `notify`** — premier vrai contact avec channels async, très formateur.
4. **Fichier de config avec `serde`** — serde est incontournable en Rust.
5. **Sources GitHub/HTTP avec `reqwest` (ou `ureq`)** — HTTP, et async si reqwest.
6. **Stash mode** — en dernier, c'est un projet dans le projet.
