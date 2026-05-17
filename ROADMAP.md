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

- [x] **Raccourcis clavier** : `u/d` (demi-page), `b/f` (page), `r` (reload), `?` (toggle help) — faits dans `pager.rs:on_key`.
- [ ] **Touche `e` edit externe** (`ui/editor.go`) : lance `$EDITOR` sur le fichier, recharge en sortie.
- [x] **Touche `c` copy** : copie le markdown brut dans le presse-papier via `wl-copy`/`xclip` en sous-process (persiste après quit).
- [x] **Détection auto de stdin** : `cat README.md | mdreader` sans argument — fait via `std::io::IsTerminal`.
- [ ] **Mode pager** (`main.go:318-334`) : flag `-p` qui délègue à `$PAGER` / `less -r` au lieu du TUI.
- [x] **Frontmatter YAML strip** : blocs `---...---` retirés avant parsing.
- [x] **Barre de statut avec % scroll** : affichée dans `pager.rs:draw`.

## Configuration & thèmes (effort moyen)

- [x] **Fichier de config** : TOML dans `$XDG_CONFIG_HOME/mdreader/config.toml` (`theme` + `width`). CLI > config > defaults. Crates : `serde` + `toml` + `dirs`.
- [x] **Thèmes syntect multiples** : flag `--theme` exposant les presets de `ThemeSet::load_defaults()`. Auto-détection clair/sombre via `termbg` plus tard.

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
