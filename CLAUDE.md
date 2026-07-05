# Avion Messager (natif Windows)

Overlay tray Windows : avion pixel art + banderole annonçant la prochaine réunion
Google Agenda. **Rust natif** (Win32 via `windows-rs`) — pas de WebView, pas de tokio
(threads std + `ureq` bloquant).

## Commandes

```powershell
cargo test                 # logique pure — doit rester 100 % verte
cargo test -- --ignored    # + test keychain (touche le Credential Manager)
cargo build --release      # exe unique ; échoue « accès refusé » si l'app tourne :
                           #   Stop-Process -Name avion-messager d'abord
cargo run -- --fly "Texte" # recette rapide : un vol, puis exit (sans client_config)
wix build wix/main.wxs -ext WixToolset.Util.wixext -o target/wix/AvionMessager.msi
# variante interne (+ identifiants) : ajouter -d IncludeClientConfig avec
# client_config.json à la racine (git-ignoré). NE JAMAIS PUBLIER ce MSI interne.
```

## Règles métier

La spec normative est `../avion-messager/docs/SPECIFICATION.md` §4 (dépôt frère).
Écarts assumés ici : réunions cliquables (lien Meet), vol sur l'écran du curseur,
clic gauche = vol manuel, fenêtre overlay à la taille du rig (pas plein écran).

**Contrat des fonctions de règle : `now` est TOUJOURS un paramètre** — jamais
`Local::now()`/`Instant::now()` dans `calendar.rs`, `scheduler.rs`, `anim.rs`
(horloge injectée, testabilité). Les exemples chiffrés §10 de la spec sont des
tests nommés : ne pas les « corriger ».

## Architecture (qui fait quoi)

- `main.rs` — tout le câblage : `AppState` (un seul `Mutex`), fenêtre message-only
  (`WM_APP_PASSAGE/MENU/NOTIFY`), thread scheduler 60 s (**1er tick immédiat**,
  fetch ≤ 5 min, early-return si déconnecté), handlers tray/menu.
- `calendar.rs`, `scheduler.rs`, `anim.rs`, `pkce.rs` — logique pure testée.
- `auth.rs` — OAuth PKCE loopback. L'accept est en **boucle** (les préconnexions
  muettes de Chrome doivent être ignorées) ; l'échange de code se fait **avant**
  d'écrire la page (elle ne doit jamais annoncer un succès non acquis).
- `overlay_win.rs` — layered window composée **une fois** (`UpdateLayeredWindow`),
  animée par `SetWindowPos` ; `Box` dans `GWLP_USERDATA`, libéré dans `WM_DESTROY`.
- `sprite.rs` — rig en rectangles, texte via `ab_glyph` + polices système
  (`consolab.ttf` → `courbd.ttf` → `cour.ttf`) ; buffer **BGRA prémultiplié**.
- `tray.rs` — description pure du menu (`menu_items`, testée) ; mapping muda dans
  `main.rs`.

## Pièges

- **Jamais de token dans les logs** ; le refresh token ne vit qu'au trousseau
  (`token_store`), l'access token en mémoire.
- **Ne pas tenir le `Mutex` pendant** `fly()`, HTTP, keychain : `drop(st)` avant
  tout `post_msg` (voir les patterns existants dans `tick`/`wire_menu_events`).
- `TrayIcon` est `!Send` → `thread_local` ; toute l'UI (tray, overlay, toast) vit
  sur le thread principal, les autres threads ne font que `PostMessageW`.
- `#![windows_subsystem = "windows"]` en release : pas de console → les logs ne
  sont visibles qu'en debug ou via `$env:RUST_LOG="info"` dans un terminal (debug).
- Version MSI (`wix/main.wxs`) = version `Cargo.toml` : bumper les deux ensemble.
- GUID des composants WiX : explicites (WIX0230 interdit les GUID auto avec
  keypath registre + fichier) — ne pas les « simplifier ».
- Autostart gaté release (`cfg!(debug_assertions)`) : un build debug n'écrit pas
  la clé Run.
