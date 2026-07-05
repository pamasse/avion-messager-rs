# Avion Messager (réimplémentation native Windows)

Overlay desktop : un avion pixel art traverse l'écran en tirant une banderole annonçant la
prochaine réunion Google Agenda, ~10 min avant chaque réunion et à la demande depuis le
menu de la barre système (tray).

Cette version est une **réimplémentation native Windows** (Win32 + `windows-rs`, sans
WebView ni runtime JS) du projet `avion-messager` (Tauri v2, cœur Rust + WebView système),
visant une empreinte disque/mémoire minimale. Voir le dépôt frère `avion-messager` pour la
version Tauri (multi-plateforme Windows/macOS).

## Build

```powershell
cargo build --release
```

Binaire produit : `target\release\avion-messager.exe`.

## Installateur MSI

Prérequis (une fois) : `dotnet tool install --global wix --version 5.0.2`.

```powershell
cargo build --release
wix build wix/main.wxs -o target/wix/AvionMessager.msi
```

MSI **par utilisateur** (aucun droit admin) : installe l'exe dans
`%LOCALAPPDATA%\Programs\Avion Messager\` + raccourci menu Démarrer. Désinstallation
via « Applications installées ». Pas de mise à jour automatique (choix assumé) : une
version plus récente du MSI remplace simplement l'ancienne (`MajorUpgrade`).

**Build interne** (pour distribuer à une équipe sans que chacun crée son client OAuth) :
placer `client_config.json` à la racine du repo (git-ignoré), puis

```powershell
wix build wix/main.wxs -d IncludeClientConfig -o target/wix/AvionMessager-interne.msi
```

Cette variante dépose aussi les identifiants dans `%APPDATA%\com.pierre.avionmessager\`.
⚠️ Ne **jamais** publier ce MSI-là (release GitHub, site…) : il contient le
`client_secret`. Le MSI public, lui, n'embarque que l'exe.

## Configuration Google OAuth — `client_config.json`

Client OAuth **type « Application de bureau »**, scope `calendar.readonly`. Créer un fichier
JSON plat (deux champs seulement — **pas** le JSON téléchargé par Google, qui enveloppe sous
`"installed"`) :

```json
{ "client_id": "…apps.googleusercontent.com", "client_secret": "GOCSPX-…" }
```

Ordre de recherche (`ClientConfig::load`, spec §4.10) :
1. `%APPDATA%\com.pierre.avionmessager\client_config.json` (build installée)
2. `.\client_config.json` (répertoire courant)
3. `..\client_config.json` (`cargo run` depuis un sous-dossier)

Sans ce fichier, l'app affiche une boîte d'erreur propre et quitte.

## Lancement

```powershell
target\release\avion-messager.exe
```

L'app tourne dans le tray (aucune fenêtre visible tant qu'aucun avion n'est déclenché).

## Réglages — `settings.json`

Stocké dans `%APPDATA%\com.pierre.avionmessager\settings.json`, modifiable depuis le menu
tray (persistant, rétrocompatible si un champ manque) :

| Champ | Défaut | Description |
|---|---|---|
| `lead_minutes` | `10` | Délai avant la réunion pour déclencher l'avion |
| `paused` | `false` | Coupe les tirs automatiques (le tir manuel reste actif) |
| `suppress_during_meeting` | `true` | Pas de tir automatique pendant une réunion en cours |
| `autostart` | `true` | Démarrage avec Windows (gaté en release uniquement) |

## Performance

Mesuré sur Windows 11, build `cargo build --release` (`opt-level = "z"`, `lto = true`,
`strip = true`) :

| Mesure | Valeur | Objectif | Résultat |
|---|---|---|---|
| Taille binaire | **2,80 Mo** (2 941 952 o) | < 5 Mo | ✅ atteint |
| RAM au repos (`WorkingSet64`, ~1 min après lancement) | **23,05 Mo** (24 166 400 o) | < 10 Mo | ❌ non atteint |
| Mémoire privée (`PrivateMemorySize64`) | 1,75 Mo (1 835 008 o) | — | pour référence |
| Processus enfants | Aucun (le build release utilise `windows_subsystem = "windows"` — pas de fenêtre console ni de `conhost.exe` ; le build debug conserve la console pour les logs) | 0 | ✅ |

Commandes utilisées :

```powershell
(Get-Item target\release\avion-messager.exe).Length / 1MB
Get-Process avion-messager | Select-Object WorkingSet64, PrivateMemorySize64
```

Le `WorkingSet64` inclut les pages partagées des DLL système Windows (GDI, COM, WinRT…)
chargées par `windows-rs`, `tray-icon` et la notification Toast ; c'est ce qui explique
l'écart avec l'objectif de conception. Piste d'optimisation non explorée dans cette tâche :
mesurer la part imputable à `tauri-winrt-notification`/COM vs. le reste.

## Recette manuelle (à dérouler par un humain — nécessite compte Google + inspection visuelle)

- [ ] 1. `cargo run` sans `client_config.json` → boîte d'erreur propre.
- [ ] 2. Avec config : tray présent, menu ordre §4.8 exact.
- [ ] 3. Connexion Google : navigateur → consentement → page « tu peux fermer cet onglet »
      → menu passe à « Se déconnecter » + réunions listées. Double-clic rapide sur
      « Se connecter » → un seul navigateur (single-flight).
- [ ] 4. « Faire passer l'avion » : traversée ~12 s, transparent, click-through, pixel art net.
- [ ] 5. Créer une réunion test à `maintenant + 11 min` (délai 10) → passage auto ~1 min après.
- [ ] 6. « En pause » coché → pas de tir automatique ; manuel fonctionne toujours.
- [ ] 7. Réunion en cours + anti-réunion coché → pas de tir auto ; manuel : « Aucune réunion
      à venir » si aucune réunion future (§10 ex. 9).
- [ ] 8. Délai : passer à 2 min → coche déplacée, `settings.json` mis à jour.
- [ ] 9. Révocation (retirer l'accès sur myaccount.google.com/permissions) → au tick suivant :
      toast unique, menu « Se connecter à Google ».
- [ ] 10. Quitter → le process disparaît.

Recette rapide (sans compte Google) : `cargo run -- --fly "Texte"` joue un seul vol avec ce texte puis quitte.

## Licence

MIT.
