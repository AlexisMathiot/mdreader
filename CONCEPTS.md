# Concepts Rust utilisés dans `mdreader`

Notes de session — récap des patterns Rust qu'on a touchés en construisant le viewer.

---

## 1. Gestion d'erreurs : `Result`, `?`, `anyhow`

Toute fonction qui peut échouer retourne `Result<T, E>`. L'opérateur `?` propage l'erreur automatiquement.

```rust
let content = fs::read_to_string(&path)
    .with_context(|| format!("lecture de {}", path.display()))?;
```

- `fs::read_to_string` retourne `Result<String, io::Error>`
- `?` : si erreur → return immédiat avec l'erreur. Sinon, déballe la `String`.
- `.with_context(...)` (de `anyhow`) : ajoute un message d'erreur lisible

### `anyhow` vs `thiserror`

- **`anyhow`** : type d'erreur "fourre-tout" pour les binaires. Pratique, peu de cérémonie.
- **`thiserror`** : pour les bibliothèques. Tu définis tes propres types d'erreur, l'utilisateur peut les matcher.

Dans `main.rs` : `fn main() -> Result<()>` — Rust accepte qu'un binaire retourne un Result.

---

## 2. Références : `&`, `&mut`, déréférencement

Une référence pointe vers une valeur qui vit ailleurs.

```rust
fn trim_trailing_space(s: &mut String, w: &mut usize) {
    while s.ends_with(' ') {
        s.pop();                           // .pop() déréf auto via .
        *w = w.saturating_sub(1);          // *w explicite à gauche du =
    }
}
```

| Côté | Forme | Pourquoi |
|------|-------|----------|
| Lecture via `.` | `w.saturating_sub(1)` | Auto-deref par l'opérateur `.` |
| Écriture (gauche du `=`) | `*w = ...` | Pas d'auto-deref pour `=`, faut le faire main |
| Lecture brute | `*w + 1` | Sans `.`, faut déréférencer toi-même |

**Règle mentale** : `&mut T` = "j'ai le droit exclusif de modifier ça" — un seul à la fois (interdit d'avoir deux `&mut` au même endroit).

---

## 3. Arithmétique sûre : `saturating_*`, `checked_*`, `wrapping_*`

Les entiers non signés (`usize`, `u16`, etc.) ne peuvent pas devenir négatifs.

```rust
let scroll: u16 = 0;
scroll - 1;                    // ❌ panic en debug, wrap à u16::MAX en release
scroll.saturating_sub(1);      // ✅ clamp à 0
scroll.checked_sub(1);         // ✅ retourne Option<u16> (None si underflow)
scroll.wrapping_sub(1);        // 🟡 wrap explicite (utile parfois pour modulo)
```

Utilisé partout dans le scroll, le wrap CJK, le calcul de largeur.

---

## 4. Pattern matching : `match`, `if let`

```rust
match code {
    KeyCode::Char('q') => self.should_quit = true,
    KeyCode::Char('j') | KeyCode::Down => self.scroll = self.scroll.saturating_add(1),
    KeyCode::PageDown | KeyCode::Char(' ') => self.scroll = self.scroll.saturating_add(10),
    _ => {}
}
```

`match` est **exhaustif** : tu dois traiter toutes les variantes (le `_ => {}` couvre le reste).

`if let` quand tu ne veux qu'une seule variante :

```rust
if let Some((lang, content)) = self.code_block.take() {
    // s'exécute seulement si Some
}
```

**Chains avec `&&`** (Rust 2024) :

```rust
if let Event::Key(key) = event::read()?
    && key.kind == KeyEventKind::Press
{
    app.on_key(key.code, key.modifiers);
}
```

---

## 5. Ownership & `mem::take`

Quand tu veux "voler" une valeur en laissant `Default::default()` à la place :

```rust
let cells = std::mem::take(&mut tb.current_cells);
// tb.current_cells est maintenant Vec::new() (default)
// tu as la valeur owned dans `cells`
```

Pratique quand tu as une référence mutable mais besoin d'owned data. Évite le clone.

Utilisé dans le `TableBuilder` pour transférer les cellules d'une row au stockage final.

---

## 6. Style stack (pattern push/pop pour état hiérarchique)

Pour gérer des styles imbriqués (gras dans italique dans blockquote) :

```rust
fn push_style(&mut self, patch: Style) {
    self.style_stack.push(self.style);   // sauvegarde l'état précédent
    self.style = self.style.patch(patch); // applique la modif
}

fn pop_style(&mut self) {
    if let Some(s) = self.style_stack.pop() {
        self.style = s;                  // restaure l'état précédent
    }
}
```

Pattern classique pour tout état hiérarchique : XML parsing, AST traversal, color stack en graphisme, etc.

---

## 7. Itérateurs : `iter()`, `map()`, `collect()`, `sum()`, etc.

Rust pousse à raisonner en chaînes d'itérateurs au lieu de boucles indexées.

```rust
let widths: Vec<usize> = max_widths
    .into_iter()                    // consomme le Vec
    .map(|w| w.max(MIN_COL_WIDTH))  // transforme chaque élément
    .collect();                     // re-construit un Vec
```

Autres méthodes utilisées :
- `.sum::<usize>()` — somme
- `.max()` — max (retourne Option)
- `.position(|&w| w == max_w)` — index du premier qui matche
- `.iter().enumerate()` — index + valeur
- `.zip(other.iter())` — paire deux iterators

`into_iter()` consomme, `iter()` emprunte, `iter_mut()` emprunte mutable.

---

## 8. Closures

Fonctions anonymes capturant l'environnement.

```rust
let make_border = |left: char, mid: char, right: char| -> Line<'static> {
    let mut s = String::new();
    s.push(left);
    for (i, &w) in widths.iter().enumerate() {
        if i > 0 { s.push(mid); }
        for _ in 0..(w + 2) { s.push('─'); }
    }
    s.push(right);
    Line::from(Span::styled(s, border_style))
};
```

`widths` et `border_style` sont **capturés** depuis le scope englobant.

Trois traits possibles :
- `Fn` : peut être appelée plusieurs fois, ne modifie rien
- `FnMut` : peut modifier ce qu'elle capture
- `FnOnce` : ne peut être appelée qu'une fois (consomme ce qu'elle capture)

---

## 9. Builder pattern

Beaucoup d'API ratatui prennent `self` par valeur et retournent `Self` :

```rust
let block = Block::default()
    .borders(Borders::ALL)
    .padding(Padding::horizontal(2))
    .title(title);
```

Ça permet de chaîner les modifications sans variables intermédiaires. Comme le builder pattern Java mais sans need d'un `.build()` final.

---

## 10. Modules

```rust
// dans main.rs
mod render;                   // déclare le module render

// dans render.rs
pub fn render(...) {}         // exposé hors du module
struct Renderer { ... }       // privé au module
```

Convention :
- `pub fn`/`pub struct` : accessible de l'extérieur
- Sans `pub` : visible uniquement dans le module

---

## 11. Cache invalidation pattern

Pour ne pas re-calculer ce qui n'a pas changé :

```rust
fn ensure_rendered(&mut self, width: u16) {
    if width != self.last_width {
        let lines = render::render(&self.content, width as usize);
        self.text = Text::from(lines);
        self.last_width = width;
    }
}
```

Idéal quand le calcul est cher (parsing markdown ici) et qu'il dépend d'inputs identifiables (la largeur).

---

## 12. Cleanup garanti (RAII pattern adapté)

Pour s'assurer qu'on restaure le terminal même en cas d'erreur :

```rust
let res = run(&mut terminal, &mut app);    // peut échouer

disable_raw_mode()?;                       // toujours exécuté
terminal.backend_mut().execute(LeaveAlternateScreen)?;
terminal.show_cursor()?;

res                                        // retourne le résultat
```

Variante plus propre : utiliser un `struct` avec un `Drop` impl qui fait le cleanup. Mais cette approche linéaire suffit pour un MVP.

---

## 13. Lifetimes : `'static`, `'a`

```rust
pub fn render(markdown: &str, max_width: usize) -> Vec<Line<'static>>
```

`Line<'static>` = "cette `Line` ne tient aucune référence vers des données qui pourraient mourir avant la fin du programme". On l'obtient en clonant les strings (`.to_string()`) au lieu de garder des `&str`.

`'static` = "vit aussi longtemps que le programme". Strings literals (`"hello"`) sont `&'static str`.

---

## 14. `Vec`, `String`, slices

| Owned | Borrowed |
|-------|----------|
| `String` | `&str` |
| `Vec<T>` | `&[T]` |
| `PathBuf` | `&Path` |

Convention API : prends `&str`/`&[T]` en paramètre, retourne `String`/`Vec<T>` quand tu produits une nouvelle valeur.

---

## 15. `Option<T>`

L'absence de `null` en Rust. Force à gérer le cas "rien" :

```rust
let max_w = *widths.iter().max().unwrap_or(&0);
```

- `.unwrap()` : panic si None (à éviter en prod)
- `.unwrap_or(default)` : valeur par défaut si None
- `if let Some(x) = opt` : pattern match
- `?` : propage None comme erreur (avec `Option`)

---

## Pour aller plus loin

- **Le Rust Book** (gratuit) : <https://doc.rust-lang.org/book/>
- **Rust by Example** : <https://doc.rust-lang.org/rust-by-example/>
- **Effective Rust** (Mara Bos) : patterns idiomatiques avancés
- **Le code de `mdcat`/`bat`** : très bonne lecture, même style de projet

Pour réviser ce projet : commence par `main.rs` (event loop), puis `render.rs` du haut vers le bas (renderer, puis layout_table, puis wrap_cjk).
