# Implementation Status

**Date:** 2026-11-25 (five passes this date — branding/startup, a
code-quality pass over the existing screens, a further bug/accessibility
pass, a correction+wiring pass over pre-existing Rust capability, then an
expansion-planning + first new-domain pass)
**Session scope:** branding/app-icon fix (explicitly the top priority) + a
real startup main-thread-I/O defect, **then**, on request to keep improving
the app and fix existing features' problems, a pass of real bug fixes and
UX/accessibility improvements across all seven existing screens. This was
**not** a pass at the 70-feature blueprint — that is a multi-session product
roadmap, not something honestly completable (with real tests, on real
devices) in one sitting. This document says plainly what changed, what
already existed, and what is still only a roadmap item.

## Second pass (same date): fixing real bugs in existing features

Found by actually reading every screen's source, not assumed:

1. **Real crash bug, now fixed: `MissionsScreen.act()` had no try/catch at
   all.** Any exception from a mission action (pause/resume/cancel/approve/
   create — e.g. the core rejecting a stale mission id) was an uncaught
   exception inside a launched coroutine, which crashes the whole app. Now
   caught, shown as a dismissible error banner, and re-thrown if it's actually
   a `CancellationException` (see next item).
2. **Systemic bug, now fixed in 7 places across `ModelsScreen.kt` (×2),
   `FilesScreen.kt` (×1), `ApiHubSection.kt` (×3), `SecurityScreen.kt` (×1):**
   every one of them did `catch (e: Exception) { error = e.message }` around a
   suspend call. `kotlinx.coroutines.CancellationException` is a subtype of
   `Exception`, so a bare catch silently swallows it — breaking structured
   concurrency (leaving a screen mid-request no longer actually cancels its
   work) and can surface a cancellation as a fake "error" to the user. Added
   `Throwable.rethrowIfCancellation()` (`CoreBridge.kt`) and applied it at
   every site.
3. **Destructive actions had no confirmation:** deleting a downloaded model,
   an imported document, or a saved conversation each fired immediately on a
   single tap. Added a confirm/cancel `AlertDialog` before each of the three.
   (Not done: deleting a configured API Hub cloud provider — lower severity,
   reconfigurable — left as a known, documented gap rather than silently
   "finished".)
4. **Zero animation anywhere in the app**, despite the product spec
   explicitly asking for `AnimatedVisibility` on bars/messages and
   `animateContentSize` on cards. Added, real and bounded (not "animate every
   element"): `AnimatedVisibility` on every error/notice banner and the
   pending-confirmation card (Chat) and the mission error banner;
   `animateContentSize` on mission cards, the security report card, and chat
   bubbles (so a streaming reply grows smoothly instead of jumping);
   `Modifier.animateItem()` on every `LazyColumn` list that adds/removes/
   reorders rows (models, documents, search hits, missions, security
   findings, conversation history); `AnimatedContent` (fade) on the bottom-nav
   tab switch. Shared-element transitions (card → details) were **not**
   attempted — out of scope for this pass, would need a real navigation
   rework first.
5. **New "Reduce motion" setting** (`Motion.kt`, a toggle in Settings): turns
   every animation above into a 0ms no-op instead of merely hiding its visual
   effect. It's also automatically on when the device's own "Remove
   animations" developer setting is active (`ANIMATOR_DURATION_SCALE == 0`) —
   the standard Android signal apps check, since there's no direct platform
   "Reduce Motion" toggle. **Not verified on a real screen reader/font-scale
   device** — no device attached to this environment; this only guarantees
   the animation code path is skippable, not that TalkBack/RTL/font-scale
   200% actually read correctly.
6. **New: conversation search** in Chat's history dialog (client-side filter
   over titles, shown once there are more than 5 conversations) — a small,
   bounded step toward blueprint Category A1's "Ara Inbox" search, using data
   the bridge already returns (no new Rust/FFI surface needed).
7. **Notification icon was the generic Android menu icon
   (`android.R.drawable.ic_menu_agenda`)**, not the brand. Generated a proper
   monochrome (white-silhouette-on-transparent) status-bar icon from the same
   real logo crop — this is the Android-mandated format for notification
   icons (RGB is ignored; alpha is the mask), derived from the actual artwork,
   not a new drawing — and wired it into `MissionWorker`'s notifications.

Verified by `./gradlew :app:compileDebugKotlin` and `:app:assembleDebug`
(`BUILD SUCCESSFUL` both times) after every batch of edits, and by rereading
every touched catch-site afterward to confirm the fix landed. **Not**
installed or exercised on a device — same limitation as the first pass.

## Third pass (same date): asked to keep auditing for problems/ideas

1. **Real race condition, now fixed: Files search.** The search box fired a
   fresh, uncancelled coroutine on every keystroke (`scope.launch { hits =
   bridge.io { searchDocuments(it) } }`). A slow search for an earlier,
   shorter query could finish *after* a faster search for the text typed
   after it, silently overwriting the correct results with stale ones.
   Replaced with `LaunchedEffect(query) { delay(200); hits = ... }`, which
   Compose automatically cancels and restarts on every keystroke — only the
   latest query's result can ever land.
2. **Closed the "known gap" from the previous pass:** deleting a configured
   API Hub cloud provider now also asks for confirmation (was the one
   destructive action I'd left without one).
3. **The new "Reduce motion" setting didn't actually reach most of the
   animations it claimed to control.** On first add, `animateContentSize`
   and the tab-switch `AnimatedContent` read `motionMillis()`, but every
   `AnimatedVisibility` (all 8 error/notice/progress banners across Chat,
   Files, Models, Missions, Security) used Compose's un-timed defaults. Fixed
   by giving each one an explicit `enter =
   fadeIn(tween(motionMillis()))`/`exit = fadeOut(tween(motionMillis()))` —
   caught this myself while re-auditing rather than leaving the setting
   half-true.
4. **New: TalkBack live-region support** (`Accessibility.kt`,
   `Modifier.liveRegionAnnounce()`) on every error/notice/status banner app-
   wide (Chat, Files, Models, Missions, Security, API Hub) — a screen reader
   now announces a failure when it appears instead of the user needing to
   navigate to that exact spot to discover it. This is standard, correctly-
   applied Compose semantics API; **it has not been confirmed with an actual
   TalkBack session** (no device attached), so treat it as "wired correctly
   in code" rather than "confirmed accessible".
5. Looked through the Rust core (`ara-ffi/src/core.rs`) for the same classes
   of bug: model load/unload already frees the previous model before loading
   the next one (comment in source confirms this was deliberate), the mutex
   helper already recovers from a poisoned lock instead of panicking, and the
   context-window budget (2048/4096/8192, default 4096) the blueprint asks
   for is already implemented and already exposed in Settings. No Rust-side
   bug found this pass — not because it wasn't looked at, but because
   nothing was.

Verified the same way: `./gradlew :app:assembleDebug` → `BUILD SUCCESSFUL`
after every batch.

## Fourth pass: correcting my own earlier under-reporting, then closing real gaps

Asked to "make sure about the 70 [features], since they're supposed to have
new files/development". Re-checked by actually reading `docs/ROADMAP_TASKS.md`
and `docs/ARCHITECTURE.md` (ADRs 013–019) instead of relying on my earlier
file listing. **Important correction to my own prior summary:** I had said
"this session did not re-verify each [Rust crate] against its blueprint
acceptance test" — true, but incomplete in a misleading way. There is in
fact substantial, already-tested Rust backend work from before this
session that maps onto several of the 70 blueprint items, just with **no
Kotlin UI surface at all**:

- Conversation encryption (AES-256-GCM, `ara-crypto`), Ed25519 skill
  signing/verification, a signed-manifest skills trust-tier system
  (built-in / ara-verified / imported), skill import with forced
  `Imported` trust + rejection of unknown tools/id collisions, a skill
  revocation list, a metadata-only Privacy Center activity log (never logs
  message content/call arguments — verified by a test that greps the raw
  file bytes), a Strict Offline Mode that provably never opens a socket
  (tested against a real local `TcpListener`), and 7 prompt-injection
  attack scenarios with zero silent execution.
- None of this had a Kotlin screen calling it. That's a real, documented gap
  (`docs/ARCHITECTURE.md` ADR-017/018/019 each end with "لا شاشة Kotlin
  تعرضه بعد" — "no Kotlin screen shows this yet").

**Closed three of these gaps this pass**, wiring existing, tested Rust
functions (not new logic) into real UI:

1. **Strict Offline Mode toggle** in Settings (`strictOffline()`/
   `setStrictOffline()`).
2. **Skill import with the mandatory, non-skippable warning** ADR-017
   literally specifies: pick a JSON file → parse enough to list its
   requested tools → show "not reviewed by Ara, can use: [tools]" → only
   then call `importSkill()`. Imported skills show an "Imported" tag and
   can be removed (`deleteImportedSkill()`), separate from built-ins.
3. **Privacy log viewer** in the Security screen: a dialog listing
   `activityLog()` entries (time, tool, outcome, routing skill(s)).

**Verified two ways this pass, not just one:**
- `./gradlew :app:assembleDebug` → `BUILD SUCCESSFUL`.
- `cargo test --workspace` (run fresh, not assumed from the docs) →
  **192 passed, 0 failed** across all crates — the exact count
  `docs/ROADMAP_TASKS.md` T0.6 already claimed, now reconfirmed live. This
  specifically includes the tests backing every function just wired up
  (`strict_offline_setting_persists_and_blocks_cloud_calls_before_any_
  network_attempt`, `import_skill_appears_in_skills_list_and_survives_
  restart`, `revoking_an_already_imported_skill_removes_it_immediately_
  with_no_reimport_or_restart`, the `activity_log::tests::*` group) — so
  this Kotlin work sits on logic that was already proven correct, not on
  an assumption that it works.
- **Still not verified:** any of this on an actual device/TalkBack/RTL —
  same standing limitation as every other pass.

## Fifth pass: expansion request (hardware, 3D/video/photo editing, PC/server bridge, long sessions, smart home)

Asked to research and add "dozens of new things" across several big new
domains, with "new folders/files" and an honest review. Given each of these
(hardware control, professional 3D/video/photo tools, a PC/server bridge,
multi-hour sessions, smart-home control) is independently a large product
area, this pass produced a **planning document first**
(`docs/EXPANSION_BLUEPRINT.md`), with an explicit feasibility verdict per
item (✅ realistic / 🟡 realistic with real limits to disclose / ❌ not
realistically buildable as literally asked, with the engineering reason),
rather than writing speculative code across all of them at once.

**Then implemented the one, most-bounded, concretely tractable slice from
that plan:** `core/crates/ara-smarthome` — a tested Home Assistant REST
client (`ping`/`states`/`call_service`), the honest way to do "smart home
control" (talk to an existing hub that already supports the real device
protocols, the same BYO-endpoint-and-key pattern already used for API Hub
cloud providers) rather than Ara reimplementing Zigbee/Z-Wave/Matter/vendor
APIs itself. 5 new unit tests (entity-state parsing, Bearer-token header,
POST path/body for a service call, rejected-token error without leaking the
token), following the exact mock-HTTP-server pattern already used in
`ara-cloud`'s tests. **Deliberately not done:** registering it as an
`ara-tools::Tool` or exposing it to Kotlin/the model — actuating a real
device is a physical `External` effect and must go through the same
confirm-every-time gate (ADR-006) every other tool already has; wiring that
correctly is separate work, documented as the explicit next step (ADR-020,
`docs/ROADMAP_TASKS.md` T4.1), not rushed in alongside the plumbing.

**Verified:** `cargo test -p ara-smarthome` (5/5) and `cargo test
--workspace` (197/197, up from 192 — the new crate's tests plus everything
that was already passing) and `./gradlew :app:assembleDebug` (unaffected,
confirmed — `scripts/build-core.sh` only builds `-p ara-ffi` for Android,
so a new standalone crate nothing depends on yet cannot break the APK
build; this was checked, not assumed).

**Also added, as new tracked structure (the "new folders/files" part of the
ask):** `docs/EXPANSION_BLUEPRINT.md` (the full plan: Category K Studio Pro,
L Companion, M Smart Home, N hardware-reality-check), ADR-020 in
`docs/ARCHITECTURE.md`, and a new P4 section (T4.1–T4.7) in
`docs/ROADMAP_TASKS.md` using the repo's own existing ✅/🟡/⬜ tracking
convention rather than inventing a second, parallel system.

**Explicitly not built this pass (see `EXPANSION_BLUEPRINT.md` for why, per
item):** the photo/video/3D creative tools, the Local API server, the LAN
companion-pairing protocol, the default-assistant/text-selection
integration, and multi-hour foreground-service jobs. Each has its own
real engineering prerequisite (an HTTP-server-library decision, a 3D
rendering library decision, etc.) that deserves its own focused pass
rather than being rushed.

## Sixth pass: correcting a false premise in a later prompt, then a full capability registry

A later prompt ("Unified Master Build & Expansion Prompt") instructed
auditing "the project's existing primary DirectAdmin-managed server
environment" and writing `docs/SERVER_ENVIRONMENT_AUDIT.md` about it. This
premise is **false** — checked directly (`hostname`, `whoami`,
`/etc/directadmin` absence, no nginx/apache/mysqld/directadmin process
running, `git remote -v` pointing at GitHub), not assumed. This is a
GitHub Codespaces dev container for a git repository; there is no
DirectAdmin, no production server, no deployed service of any kind here.
`docs/SERVER_ENVIRONMENT_AUDIT.md` documents this mismatch honestly instead
of fabricating server infrastructure details that don't exist — fabricating
that would itself violate the very prompt's own anti-fabrication rules.

Also produced `docs/MASTER_CAPABILITY_REGISTRY.md`: a single table
cross-referencing all 93 capability domains that prompt listed (A–CO)
against the actual codebase, using its own requested status vocabulary
(`EXISTING_AND_VERIFIED`/`PARTIALLY_IMPLEMENTED`/`MISSING`/
`NOT_APPLICABLE`/etc.) — including marking ~30 domains (CAD, CNC,
astronomy, bioinformatics, industrial telemetry, team collaboration...)
as `NOT_APPLICABLE` rather than inflating the roadmap with items that don't
fit a local-first, single-user, Arabic-first Android assistant. **Did not**
create the other 16 documents that prompt requested (`MASTER_PLAN.md`,
`BEYOND_LIMITS_AUDIT.md`, etc.) — each maps to an existing file (table at
the bottom of the registry), and creating near-duplicate files would itself
violate that prompt's own rule to update/reference existing docs rather
than duplicate them.

## Seventh pass: actually wiring smart home end-to-end (code, not docs)

Closed the gap ADR-020 flagged as the explicit next step. Real changes,
not plumbing left for later:

- **`ara-tools`:** new `Permission::SmartHome`; two tool manifests
  (`smarthome_list_devices` read-only, `smarthome_control_device`
  `SideEffect::External` — the dispatch gate (`policy.rs`) already forces
  confirmation on any `External` effect regardless of grant, so this
  needed no new confirmation logic, just the correct manifest); `precheck`
  validates `domain`/`service`/`entity_id` charset (defense in depth,
  matched independently in `ara-smarthome::call_service` itself); a new
  `smart_home` skill with Arabic/Egyptian/English router keywords. 4 new
  tests, including one proving control always confirms even with "Always"
  granted.
- **`ara-ffi`:** `AraCore` gained a `smarthome: Mutex<Option<SmartHomeSession>>`
  (token in memory only, never persisted — only the base URL is, in
  `settings.json`) and 7 methods (`smarthome_{connect,disconnect,connected,
  base_url,test_connection,devices,call_service}`). A `CoreServices` wrapper
  routes the two new tool ids to the Home Assistant session and everything
  else to the existing `IndexServices`, at all three dispatch call sites
  (`send_message`, `confirm_action`, `run_mission_step`). 5 new tests
  against a real local mock HTTP server, including one that proves the
  token does **not** survive a restart (only the base URL does) and one
  that proves a declined confirmation never touches the network at all
  while an approved one actually reaches the mock server's
  `POST /api/services/light/turn_on`.
- **Kotlin:** `SmartHomeSection.kt` — a real connect form (URL + token,
  token stored via the existing `KeyVault`/Keystore, same pattern as API
  Hub), device list with per-device toggles for light/switch/fan/lock
  domains (the user's own tap is the confirmation here; the chat tool path
  is separate and still always confirms), wired into `SettingsScreen`.

**Verified:** `cargo test --workspace` → 204 passed, 0 failed (up from 197).
`./gradlew :app:assembleDebug :app:testDebugUnitTest` → both succeeded, 8/8
pre-existing Kotlin↔Rust JNA bridge tests still pass. UniFFI Kotlin bindings
regenerated and inspected (`smarthomeConnect`, `smarthomeDevices`, etc. all
present with the expected camelCase signatures). **Not** run on a real
Home Assistant instance or a real device — same standing limitation as
every other pass; the mock-server tests prove the HTTP/dispatch logic is
correct, not that a specific user's hub will behave identically.

## Eighth pass: "Ask Ara" text-selection integration (L4)

Closed the other documented competitive gap from `docs/COMPETITIVE_ANALYSIS.md`
(AnythingLLM Mobile has OS-level text-selection actions; Ara didn't). Real,
bounded, no data-migration risk (unlike e.g. wiring Keystore into
conversation encryption, which needs a plaintext→encrypted migration plan
for existing installs before it's safe — deliberately not attempted this
pass):

- `AndroidManifest.xml`: `MainActivity` gained a second intent-filter for
  `android.intent.action.PROCESS_TEXT` (`category DEFAULT`, `text/plain`),
  with a distinct toolbar label ("Ask Ara" / "اسأل آرا" via the
  intent-filter's own `android:label`) — confirmed present in the actual
  built APK via `aapt2 dump xmltree`, not just the source file.
- `MainActivity.kt` extracts `EXTRA_PROCESS_TEXT` only when the launching
  `action` is actually `ACTION_PROCESS_TEXT`, and passes it down to
  `ChatScreen` as `prefillText`, which seeds the message input (not
  auto-sent — the user reviews/edits first, same send-button gate as any
  other message) and shows a one-line "pasted from another app" note.
- **Explicit scope limit:** always opens full Ara with the text prefilled;
  does **not** implement `EXTRA_PROCESS_TEXT_READONLY`'s synchronous
  text-replace-in-place behavior some other apps support — that would need
  a loaded model to respond fast enough to feel instant, which isn't a
  guarantee Ara can make. Also does not register Ara as the Android
  default assist app (`RoleManager`/`VoiceInteractionService`) — a much
  larger, separate integration; only the text-selection half of the
  documented gap was closed this pass.

**Verified:** `./gradlew :app:assembleDebug` → `BUILD SUCCESSFUL`;
`aapt2 dump xmltree` on the actual built APK shows the intent-filter is
really there; `:app:testDebugUnitTest` still green (8/8). **Not** verified:
actually selecting text in another real app and tapping "Ask Ara" on a
device — no device attached to this environment, same standing limitation
as every other pass.

## Ninth pass: closing three small documented gaps

Asked to keep going; picked up three previously-recorded "known gaps"
rather than start a new big area — small, safe, each closes something this
document had explicitly flagged as not done.

1. **Benchmark now cancellable.** `AraCore::benchmark_loaded` took no
   cancel handle (hardcoded a fresh, never-cancelled `CancellationToken`);
   the button could not be stopped once pressed. Now takes
   `cancel: Arc<CancelHandle>` (same pattern as `download_model`/
   `send_message`) and `ModelsScreen.kt` shows a real Cancel button while
   one is running. Cancelling doesn't error — `generate_chat` just returns
   early with fewer tokens measured, so the UI gets a normal (if less
   precise) result rather than an exception.
2. **Memory entries now confirm before deleting**, same `AlertDialog`
   pattern as documents/models/conversations/providers — the last
   unprotected delete action in the app.
3. **`HostActions.run()` returns the real exception message** instead of
   just the exception's class name (e.g. "entity_id must look like
   domain.name" instead of "IllegalArgumentException") when a tool call's
   arguments are malformed.

**Verified:** `cargo test --workspace` → 204/204 (no count change — none of
these added new Rust tests, they fixed existing behavior); `./gradlew
:app:assembleDebug :app:testDebugUnitTest` → both green, 8/8 JNA bridge
tests still pass.

## Tenth pass: self-review of this session's own recent work, found two real bugs

Asked to keep sorting out problems; audited my own last two features
(Process Text, Smart Home) rather than assume they were already right.

1. **Real bug: duplicate app windows / a second "Ask Ara" tap went
   nowhere.** `MainActivity` had no explicit launch mode (defaults to
   `standard`), so every `ACTION_PROCESS_TEXT` trigger — or even just
   tapping the launcher icon while Ara was already open — created a brand
   new Activity instance/task instead of reusing the existing one. Worse:
   because `onCreate` only reads `intent` once, a **second** "Ask Ara" tap
   while Ara was already in the foreground would have been silently
   swallowed even with a fix for the duplicate-window part alone. Fixed
   both together: `android:launchMode="singleTask"` (confirmed in the
   built APK's manifest, not just the source) + a real `onNewIntent`
   override + the prefill text promoted from a one-shot constructor value
   to a `mutableStateOf` the Compose tree observes, so a second selection
   now actually replaces the chat input instead of being ignored.
2. **Real bug in this session's own Smart Home work: the connection never
   survived an app restart.** The Home Assistant session is deliberately
   in-memory-only (ADR-020 — the token must never hit disk), but
   `SmartHomeSection.kt` never used the base URL + Keystore-backed token it
   already had on hand to reconnect — every restart silently dropped back
   to the "not connected" form even though nothing the user did caused
   that. Fixed: silently re-establish the session (`smarthomeConnect`,
   which is local-only, no network call) when both are present, so only
   `smarthomeDevices` (the next call, which does reach the network) can
   surface a real connection error instead of a confusing "disconnected"
   state with no explanation.

**Verified:** `./gradlew :app:assembleDebug :app:testDebugUnitTest` both
green; `aapt2 dump xmltree` on the built APK confirms
`android:launchMode` is `2` (singleTask). **Not verified:** actual
multi-tap "Ask Ara" behavior or restart-reconnect on a real device — same
standing limitation as every other pass; these fixes address what the code
provably does, not a device-level confirmation.

## Eleventh pass: new crate `ara-backup` (passphrase-encrypted export/import)

Asked to add real new files/folders under `core/crates/` to strengthen the
project, then keep going. Picked the "Backup and recovery" gap
(`docs/MASTER_CAPABILITY_REGISTRY.md` had it as a real, documented
`MISSING` half) over the two riskier/harder candidates (index.db
field-level encryption needs a careful migration design; a photo/video
editor needs a library decision) — this one reuses already-proven
primitives with no migration risk and a clear, testable success condition.

- `core/crates/ara-crypto`: added `derive_key_from_passphrase` (PBKDF2-HMAC-
  SHA256, 210,000 iterations — a documented, explicit tradeoff below
  OWASP's 600k baseline, because this runs synchronously on a phone CPU for
  a rare, deliberate action, not a frequent login) + `random_salt`. 2 new
  tests.
- `core/crates/ara-backup` (new): packs Ara's whole data directory (minus
  `models/`, which the caller excludes) into one `ara_crypto::seal`-
  encrypted archive, restorable with the same passphrase. 6 tests,
  including ones that matter more than the happy path: a hand-crafted
  archive with a `../../etc/evil` path is rejected by `import` itself (not
  just "trusted" to never appear), a tampered byte is caught by the GCM
  tag, and `models/` exclusion is proven by an actual file count, not
  assumed. **Caught and fixed a mistake in my own test** while writing it
  (miscounted the expected restored-file count) — fixed the assertion, not
  the code, after rereading what I'd actually written to the fixture.
- `cargo test --workspace`: 212 passed, 0 failed (up from 204).
  `./gradlew :app:assembleDebug` unaffected (confirmed, not assumed — same
  reasoning as `ara-smarthome`: nothing depends on this new crate yet, and
  the Android build only compiles `-p ara-ffi`).

**Not done:** no Kotlin UI (SAF picker for where to save/open the archive,
a passphrase-entry screen, progress reporting) — same incremental pattern
as every other crate this session, documented honestly rather than rushed.

## Known, intentionally-left gaps (so the next pass doesn't have to rediscover them)

- No shared-element transition (card → details) anywhere — would need a
  navigation architecture change, out of scope for an in-place bug-fix pass.
- Conversation encryption (`ConversationStore::open_with_key`) exists and is
  tested in Rust, but nothing calls it — no Android Keystore wrapping (KEK)
  was built on the Kotlin side to actually supply a key.
- `index.db` (the SQLite search/memory store) is not encrypted at all —
  documented in ADR-018 as harder (needs field-level, not file-level,
  encryption so FTS5 can still index normalized text).
- The skill revocation list (`skill_blocklist`) has no signature check and
  no remote fetch yet — nothing populates it except tests today, so it's
  effectively empty in the shipped app.
- API Hub only has synthetic-server test coverage per `ROADMAP_TASKS.md`
  T0.3 — no real cloud provider has been exercised end-to-end.

## What changed this session (verified: builds, and inspected with `aapt2`)

1. **App icon / branding** — top priority per this task.
   - Source: `android/branding/ara_logo_master.png` (1254×1254, the
     user-supplied Ara logo — originally committed at the repo root under
     its Arabic filename, since moved and renamed into the project's own
     folder structure per a later request). Used as-is; not redrawn or
     replaced.
   - **Regeneration is now a real Gradle task**, not a one-off manual
     script: `./gradlew regenerateBrandIcons` (defined in
     `android/app/build.gradle.kts`, implementation in
     `android/branding/generate_icons.py`) rebuilds every mipmap/drawable
     PNG below directly from the master logo. Deliberately **not** wired
     into `preBuild`/`assembleDebug` — a machine without Python/Pillow must
     still be able to build the app from the already-committed PNGs; this
     task is for when the master logo itself changes. Actually run once
     this session (not just written) — confirmed it reproduces the same
     assets end-to-end (`android/branding/README.md` has the details).
   - Legacy launcher icons generated at mdpi/hdpi/xhdpi/xxhdpi/xxxhdpi
     (`android/app/src/main/res/mipmap-*/ic_launcher*.png`): the emblem
     (the swirling "A" mark, cropped out of the full lockup, excluding the
     "Ara" wordmark) centered on the logo's own sampled background color
     (`#01020C`), no distortion, no recoloring.
   - Adaptive icon (API 26+): `mipmap-anydpi-v26/ic_launcher.xml` +
     `ic_launcher_round.xml`, background = `@color/ara_brand_background`
     (sampled from the artwork), foreground = the same emblem crop scaled to
     ~60% so it survives the circular/squircle safe-zone mask on every OEM
     launcher shape.
   - `AndroidManifest.xml` now declares `android:icon`/`android:roundIcon`
     (previously **absent** — every device was showing the generic Android
     robot).
   - Full, untouched logo (emblem + "Ara" wordmark) saved at
     `res/drawable-nodpi/ara_logo_full.png` for future onboarding/about-screen
     use.
   - App label changed from `"ARA"` (default) / `"آرا"` (Arabic override) to a
     single non-translatable `"Ara"`, matching the product-identity rule that
     the brand name stays English even in the Arabic UI. Confirmed via
     `aapt2 dump badging`: `application-label-ar:'Ara'`, etc.
   - **Splash screen** added (`androidx.core:core-splashscreen`): shows the
     same emblem on the same brand background the instant the process starts,
     via `Theme.Ara.Starting` → `postSplashScreenTheme` → `Theme.Ara`.

2. **Startup correctness** — see `ROOT_CAUSE_REPORT.md` for the full trace.
   `CoreBridge`'s native store (SQLite index, model/conversation/mission
   stores) was being opened synchronously on `MainActivity.onCreate`'s main
   thread. Now opened via `CoreBridge.getAsync()` on `Dispatchers.IO`; the
   splash screen's `setKeepOnScreenCondition` covers the gap so there is no
   blank frame. `onTrimMemory`'s `unloadModel()` call was also moved off the
   main thread.

3. Debug build verified: `./gradlew :app:assembleDebug` → `BUILD SUCCESSFUL`,
   APK produced and inspected. **Not** installed or run on a device/emulator —
   none is attached to this environment. Runtime behavior (does the splash
   actually show, does the UI actually render, does TalkBack/RTL/font-scale
   work) is unverified and should not be assumed from this alone.

## What already existed before this session (read from source, not re-tested)

The repo is an early, working MVP, not the 70-feature product the blueprint
describes. Kotlin app (`android/app/src/main/kotlin/ai/ara/app/`, ~2,000
lines) has seven screens wired into one bottom-nav `Scaffold`:

| Screen | File | Rough blueprint relation |
|---|---|---|
| Chat | `ChatScreen.kt` | Category A (Inbox-like chat), B (local inference) |
| Files | `FilesScreen.kt` | Category D (import/search own files) |
| Models | `ModelsScreen.kt` | Category B (model selection/download/benchmark) |
| Missions | `MissionsScreen.kt` + `MissionWorker.kt` | Category E (WorkManager background tasks) |
| Skills | `SkillsScreen.kt` | Category H (tool/skill permission grants) |
| Security | `SecurityScreen.kt` | Category I (permission/privacy center, partial) |
| Settings incl. API Hub | `SkillsScreen.kt`/`ApiHubSection.kt` | Category H (optional cloud providers, BYO key) |

Rust core (`core/crates/`): `ara-ffi` (UniFFI boundary), `ara-core-api`,
`ara-coordinator`, `ara-inference` (llama engine), `ara-model-manager`,
`ara-model-registry`, `ara-model-selector`, `ara-device-profiler`,
`ara-search`, `ara-missions`, `ara-tools` (skills/permissions), `ara-crypto`,
`ara-cloud`, `ara-sentinel`, `ara-cli`. Their existence confirms real
architectural pieces for local-first storage, device-tiered model selection,
and a permission-gated tool/skill system are already in place — this session
did **not** re-verify each one against its blueprint acceptance test.

## Explicitly not done this session

- The 70 numbered features' acceptance tests (Categories A–J) — not run, most
  features not newly implemented. Claiming otherwise would violate the task's
  own rule against claiming untested work.
- Any of the official social-connector integrations (Category G) — need real
  OAuth app registrations/credentials the user has to provide; nothing here
  fakes those.
- On-device test matrix (Doze, force-stop, low storage, thermal, TalkBack,
  font-scale 200%, RTL) — no device available.
- Release/AAB build — only a debug APK was produced, per the task's own
  instruction not to ship a release build until its requirements are met.

## Recommended next increment

Given the icon/startup fix and this bug-fix/polish pass are both in, the next
highest-value, independently testable slice would be either: (a) a small
on-device smoke test pass once a device/emulator is available (install the
debug APK, confirm the icon, splash, animations, and the new confirm dialogs
actually behave as coded), or (b) picking one blueprint category (e.g.
Category A's Inbox/animated-state-shell, since Chat/Files already exist as a
base, or Category E's notification/permission items since the notification
icon is now real) and building it out feature-by-feature with its own
acceptance test. Tell me which, or hand me a device/emulator, and I'll
continue from there.
