# Harbor Games UI and embed boundary

Source inputs:

- Harbor games platform contract
- Neo Grounds embeddable store at commit `4959256`
- Harbor local library at commit `bdfbe3e`
- Harbor Worker runtime at commit `71e333d`

## Navigation and store

**Games** is a first-class sidebar route with Store and Library tabs. Release builds compile the exact store origin `https://games.social-harbor.com`. A development build may set `VITE_HARBOR_GAMES_ORIGIN`; the page displays the override and production ignores it.

The iframe sandbox grants only scripts, forms, and same-origin state needed by the cross-origin Neo Grounds store. It grants no top navigation, popups, downloads, pointer lock, fullscreen, or Tauri capability. The store receives only the current compiled Tauri parent origin.

Harbor accepts an install intent only when:

- `event.origin` is the exact configured games origin;
- `event.source` is the current store iframe window;
- the object has exactly `gameId`, `source`, `type`, `version`, and `versionId`;
- source/type/version are the compiled constants;
- both IDs satisfy the bounded identifier grammar.

URLs, digests, permissions, package bytes, code, and extra fields invalidate the message. Harbor resolves preview metadata through its own Rust command and shows title, creator, version, size, permissions, and package digest. Confirmation causes Rust to resolve metadata again, stream the package from the compiled HTTPS API, verify metadata/package agreement and both signatures, then install it atomically.

## Local library

Users can inspect and confirm a manual `.harborgame` import. They can select a machine-visible discovery folder, scan it non-recursively, see per-file validation errors, and confirm valid packages individually. Every source is copied to profile-managed storage before play.

Library cards show source, creator, version, size, approved permissions, and play history. Uninstall has a separate opt-in for save deletion; the default preserves saves.

## Player

Selecting Play loads only the profile-managed runtime bundle. The Worker starts after the real package reaches ready state. The player provides focused keyboard/pointer input, Canvas rendering, stop/start, expand, host-owned fullscreen, crash state, and close. Closing or navigating destroys the Worker, timers, audio, and input listeners.

Installed games remain playable when the network and store are unavailable. The store tab reports offline status without representing the local library as unavailable. Packages requesting capabilities outside the current `save_data` support, including deferred multiplayer, cannot be confirmed for installation.

Tests cover keyboard navigation landmarks, the exact iframe sandbox/source, offline copy, confirmation-before-install, creator/permission display, wrong origin/source/schema/ID rejection, remote-URL rejection, compiled parent origins, Worker cleanup, and production Worker bundling.
