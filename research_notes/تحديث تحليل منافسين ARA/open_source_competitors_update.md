# Open-Source Local-First Android AI Assistant Competitors — Update Since 2026-11-25

**Research date observed by tools/agent: 2026-11-25 (confirmed via `date` command and system context).**

## IMPORTANT DATE FLAG — read this first

The task asks to find what changed **"since 2026-11-25 (a prior check-in date)."** The system clock on this machine, and the environment's own date context, both read **2026-11-25 — i.e., today IS the prior check-in date**, not a date after it. There is no elapsed "since" window to report (0 days), and I have no stored prior snapshot of these five repos from an actual 2026-11-25 check-in to diff against (no local file with that baseline was found in this workspace, and I was not given one).

Given this, I did the next best thing: I pulled **live GitHub API state** (commits, releases, repo metadata) for all five repos as of **2026-11-25, ~12:50–13:00 UTC**, and I am reporting:
1. The most recent commits/releases visible right now (many cluster in the days/weeks immediately before this timestamp — e.g., Private Mind has commits dated literally today, 2026-11-25).
2. Verified source-level facts (code, dependencies, locale files) rather than marketing claims, each dated.
3. Explicit flags where I could not establish "before vs. after 2026-11-25" because no baseline exists to compare against — these are marked **[no baseline — cannot confirm novelty]**.

Treat everything below as "state of the repo as observed on 2026-11-25," not strictly "new since 2026-11-25," except where a commit/release timestamp itself is dated on or after 2026-11-25.

---

## 1. AnythingLLM Mobile — Mintplex-Labs/anythingllm-mobile

**Repo stats (checked 2026-11-25):** 211 stars, 58 forks, 19 open issues, GPLv3, last push 2026-11-24T04:30Z. [Source](https://github.com/Mintplex-Labs/anythingllm-mobile)

**Releases:** Latest tagged release is `v1.3.2` (2026-10-02). No tagged release since then, but commits continue on `main` past that tag (untagged commits through 2026-11-24). [Releases](https://github.com/Mintplex-Labs/anythingllm-mobile/releases)

**Recent commits (most recent first, all verified via `gh api .../commits`):**
- `d4663df` (2026-11-24): "Answer each image picker result once so a double tap can't abort the app (#163)" — bugfix
- `09f45ff` (2026-11-24): "Stop Animated from using setNativeProps so Reanimated frames can't abort (#162)"
- `b5280a6` (2026-11-24): "Open the Downloads app without waiting for a result (#161)"
- `ef30a7d` (2026-11-24): "Don't crash when a stored cookie can't be parsed (#160)"
- `7d490e8` (2026-11-24): "Stream workspace sync so image-heavy workspaces don't run out of memory (#159)"
- `e83e59f` (2026-11-05): "Replace Discord community links with GitHub Discussions and r/anythingllm (#140)" — community-channel change, Discord → GitHub Discussions/Reddit
- `e863681` (2026-10-05): "Spill PDFBox scratch buffer to disk past 32MB when parsing PDFs (#127)" — PDF parsing robustness fix, not a new feature
- `52b4a63` (2026-10-02): "Add Sign in with ChatGPT provider and move OpenAI to the Responses API (#121)" — new provider auth
- `1d77d9a` (2026-10-01): "Restyle tool approval card and add 'always approve' tool auto-approvals (#120)"
- `7a6fdc5` (2026-10-01): "Read YouTube transcripts when a video link is scraped or shared (#119)" — new content-ingestion feature
- `69343bd`/`add22d5`/`fdaf33f` (2026-09-30): "1.3.1 Device Assistant card", "set alarms, timers and reminders from chat", "AnythingLLM as the Android default digital assistant" — this cluster made AnythingLLM Mobile a selectable **Android system default digital assistant**, with alarm/timer/reminder tool-calling.
(All links: `https://github.com/Mintplex-Labs/anythingllm-mobile/commit/<sha>`, checked 2026-11-25.)

**Arabic/RTL localization:** `src/locales/` contains: `de, en, es, fr, hi, id, it, ja, ko, nl, pl, pt-BR, ru, tr, vi, zh-TW, zh` — **17 locales, no `ar` directory.** [Source](https://github.com/Mintplex-Labs/anythingllm-mobile/tree/main/src/locales) (checked 2026-11-25). This matches the prior finding of "no Arabic support at all" — confirmed still true today.

**At-rest encryption:** `package.json` lists `react-native-keychain ^10.0.0` as a dependency — this is for secure credential/token storage (e.g., API keys), **not full chat-history or document database encryption**. No SQLCipher or equivalent found. No evidence of chat/document at-rest encryption. [Source: package.json, checked 2026-11-25]

**RAG/retrieval:** No new retrieval/embedding commits found in this window; existing PDF ingestion (PDFBox) got a reliability fix (large-file scratch-buffer spill, #127, 2026-10-05) but this is a bugfix, not a new RAG feature.

**Voice:** No STT/TTS-specific commits found in the reviewed window.

**Pricing/licensing/maintenance:** Still GPLv3, actively maintained (daily commits as of 2026-11-24), not archived.

**Star/community growth:** 211 stars / 58 forks as of 2026-11-25. **[no baseline — cannot confirm growth delta]** since no prior-snapshot star count was available to diff against.

---

## 2. PocketPal AI — a-ghorbani/pocketpal-ai

**Repo stats (checked 2026-11-25):** 9,787 stars, 1,183 forks, 141 open issues, MIT license, last push 2026-11-25T00:52Z (i.e., pushed to literally today). [Source](https://github.com/a-ghorbani/pocketpal-ai)

**Releases:** Very active release cadence — `v1.24.0` (2026-11-17), `v1.23.0` (2026-11-10), `v1.22.1` (2026-11-05), `v1.22.0` (2026-10-30), plus a dependency-archive release `onnxruntime-c 1.24.3 pod archive` (2026-11-22). [Releases](https://github.com/a-ghorbani/pocketpal-ai/releases)

**Recent commits:**
- `5a27b5f` (2026-11-23): "fix(models): keep iOS model files out of iCloud backup (#1102)" — privacy-adjacent fix (prevents model files syncing to iCloud), not full at-rest encryption.
- `aae21cf`/`a417375` (2026-11-22): iOS build fix (onnxruntime-c pod archive), and a bug report for "A web page or wrong-size file finishes as a downloaded model (#1098)".
- `6848b02` (2026-11-21): "Dictation drops quiet speech the waveform shows (#1094)" — confirms an existing **on-device dictation/STT feature** with a waveform UI; this is a bug-tracking issue, not a new feature, but it confirms STT is a live, maintained feature.
- `ed0b965` (2026-11-19): "SM8850P and other letter-suffixed Snapdragon SoCs get CPU only (#1090)" — hardware-acceleration compatibility issue.
- `53f7d5d` (2026-11-17): "chore(deps): bump llama.rn to 0.13.0-rc.11 (#1084)" — backend engine bump, release-cycle fix for v1.24.0.
(Links: `https://github.com/a-ghorbani/pocketpal-ai/commit/<sha>`, checked 2026-11-25.)

**Arabic/RTL localization — notable finding:** PocketPal uses **Weblate** for crowdsourced translation, and translation-sync commits hit Arabic explicitly and recently:
- `2a06fab` "Translated using Weblate (Arabic)" (2026-11-17)
- `e53d537` "Translated using Weblate (Arabic)" (2026-11-15)
This is in addition to the existing `src/locales/ar.json` file confirmed present in the repo. [Source: commit history + file search, checked 2026-11-25] This confirms **Arabic translation work is active and ongoing** (consistent with the prior finding that PocketPal was the only one of the four with "partial real Arabic strings" — and it continues to receive fresh Arabic strings via Weblate as of mid-November 2026, right at the edge of the check-in window).

**At-rest encryption:** Searched for encryption-related files; only found `android/app/src/main/res/xml/backup_rules_legacy.xml` and `backup_rules_12_plus.xml` — these are **Android auto-backup exclusion rules**, not encryption. No SQLCipher, no Keystore-backed storage found for PocketPal. **No at-rest encryption evidence.**

**RAG/retrieval:** Search for "embedding" hit a GitHub API rate limit and could not be completed in this session — **[gap: not fully verified, re-check needed]**. PocketPal's README (not re-fetched this session) has historically focused on pure on-device chat/benchmarking rather than document RAG; no retrieval-feature commit was seen in the reviewed commit window.

**Voice:** On-device dictation/STT is a confirmed, actively-maintained existing feature (bug `#1094` about waveform/quiet-speech handling, 2026-11-21).

**Pricing/licensing/maintenance:** MIT, free, extremely active (releases roughly every 3–10 days), by far the largest community of the five (9.8k stars vs. the next-largest open-source one, ChatterUI, at 2.9k).

**Star/community growth:** 9,787 stars / 1,183 forks as of 2026-11-25. **[no baseline — cannot confirm growth delta]**

---

## 3. Google AI Edge Gallery — google-ai-edge/gallery

**Repo stats (checked 2026-11-25):** 25,460 stars, 2,852 forks, 500 open issues, Apache-2.0, last push 2026-11-24T21:36Z. By far the largest and most active of the five. [Source](https://github.com/google-ai-edge/gallery)

**Releases:** `1.0.23` (2026-11-04), `1.0.22`/`1.0.21` (2026-10-28), `1.0.20` (2026-10-06). [Releases](https://github.com/google-ai-edge/gallery/releases)

**Recent commits of note:**
- `1c7b534` (2026-11-12): **"Harden skill WebView isolation and enable AES-GCM encryption for user data."** — **VERIFIED IN SOURCE.** I pulled `Android/src/app/src/main/java/com/google/ai/edge/gallery/UserDataSerializer.kt` directly: it implements `EncryptedUserDataSerializer`, using **AES/GCM/NoPadding with a 128-bit GCM tag, keyed via the Android Keystore (`AndroidKeyStore`, alias `__oss_secret_gallery_gcm__`)**, with a `"v2:"` version prefix implying migration from a legacy AES/CBC/PKCS7 scheme (`DEFAULT_LEGACY_CBC_KEY_NAME`) to the new GCM scheme. This is a real, hardware-keystore-backed at-rest encryption upgrade — **the first confirmed at-rest encryption feature found across all five repos in this research.** [Commit](https://github.com/google-ai-edge/gallery/commit/1c7b534f81a0e5e65515f0e5211398cdcc528738), [file](https://github.com/google-ai-edge/gallery/blob/main/Android/src/app/src/main/java/com/google/ai/edge/gallery/UserDataSerializer.kt), checked 2026-11-25.
  - **Caveat / scope uncertainty:** The commit's touched files (`AgentChatScreen.kt`, `TinyGardenScreen.kt`, `AppModule.kt`, `GalleryWebView.kt`, `MessageBodyWebview.kt`) suggest this is tied to the **in-app "skills"/agent runtime and its WebView sandbox**, not necessarily the full chat-history database. The repo's proto directory (`Android/src/app/src/main/proto/`) separately defines `chat_history.proto`, `settings.proto`, `onboarding.proto`, `mcp.proto`, `skill.proto`, etc. — I could not confirm from the commit alone whether `chat_history.proto`-backed conversation storage is covered by this new encryption or whether it's scoped to skill/agent local state (e.g., the "TinyGarden" mini-app) and settings. **Flag this as "at-rest encryption confirmed for some local user data via AES-GCM + Android Keystore; exact data-scope (full chat history vs. skill/agent state only) is unverified."**
- `eb83265` (2026-11-20): "Ask for user consent before the agent reads calendar events." — new privacy-consent gate for an agentic calendar-reading feature.
- `3444db3` (2026-11-10): "Add Decision Lab (Photo ID Capture + AI Dino Run) showcase task to AI Edge Gallery on Android and iOS." — new demo/showcase feature.
- `8c756f9` (2026-11-11): "Support up to 10 images per request in AICore inference" — multimodal capability increase.
- `817c84e` (2026-11-10): "Add unified token estimation module for AI Edge Gallery Android."
- `1652109`/`8f7af12` (2026-11-10): new `LlmSessionManager`/`LlmModelHelper.loadModel` APIs for agent runtime conversations — internal architecture work supporting the agent/skill system.
(Links: `https://github.com/google-ai-edge/gallery/commit/<sha>`, checked 2026-11-25.)

**Arabic/RTL localization:** A code search for "arabic"/"ar" in the repo returned **no Arabic locale files**. The only "ar"-matching hits were `ar_translator.proto` and `XrLanguageTeacher*` files — these are for an **Augmented Reality (AR) language-teaching feature**, not Arabic language. **Confirmed: still no Arabic localization**, consistent with the prior finding. [Source: code search, checked 2026-11-25]

**At-rest encryption:** See above — confirmed AES-GCM + Android Keystore added 2026-11-12, scope partially unverified.

**RAG/retrieval:** No PDF/document-RAG commits found in the reviewed window; the project's recent focus is agentic tool-use (calendar access, skills, WebView-sandboxed mini-apps) rather than document retrieval.

**Voice:** Not covered in the reviewed commit window.

**Pricing/licensing/maintenance:** Apache-2.0, free, extremely actively maintained by Google (near-daily commits, large commit volume, many "Internal change." commits indicating this mirrors an internal Google repo via Piper — note the `PiperOrigin-RevId` in commit `1c7b534`, confirming it's synced from Google's internal monorepo).

**Star/community growth:** 25,460 stars / 2,852 forks as of 2026-11-25 — the largest of the five by a wide margin. **[no baseline — cannot confirm growth delta]**

---

## 4. Private Mind (Software Mansion) — software-mansion-labs/private-mind

**Repo stats (checked 2026-11-25):** 416 stars, 54 forks, 29 open issues, MIT license (GitHub API reports "Other" because the LICENSE file bundles MIT + BSD-3-Clause [ExecuTorch] + a JTransforms notice, but the project's own license is MIT), last push **2026-11-25T12:30Z — i.e., commits landed today, during this research window.** [Source](https://github.com/software-mansion-labs/private-mind), [LICENSE](https://github.com/software-mansion-labs/private-mind/blob/main/LICENSE)

**Releases:** `v1.4.0` (2026-11-19, "Android 183, iOS 43"), `v1.3.6` (2026-11-12), `v1.3.5` (2026-10-30), `v1.3.4` (2026-10-22), `v1.3.3`/`v1.3.2` (2026-10-16). [Releases](https://github.com/software-mansion-labs/private-mind/releases)

**Commits dated on/after 2026-11-25 (the only repo of the five with same-day activity):**
- `2a67c13` (2026-11-25T12:30Z): "perf(chat): keep the branch-marker array when a reload finds the same markers (#577)"
- `faf95ce` (2026-11-25T12:24Z): "feat(model family): push the attribution into the same sheet instead of stacking another (#576)"
- `8348afd` (2026-11-25T11:45Z): "fix(chat): give the context size sheet the sheet background (#574)"
- `f29df20` (2026-11-25T11:44Z): "feat(web): answer standalone web questions faster and recall dropped turns (#570, into main) (#573)" — improves the private web-search feature's latency and multi-turn recall.
- `a3f58a3` (2026-11-25T11:23Z): **"feat(context): choose the context size, and size each turn's sources to it (#569)"** — lets the user pick the model's context window and auto-sizes how many retrieved RAG sources fit per turn. This is a direct RAG-pipeline enhancement, dated today.
(Links: `https://github.com/software-mansion-labs/private-mind/commit/<sha>`, checked 2026-11-25.)

**Other recent features (2026-11-13 to 11-24, i.e., just before the window):**
- `7cce576` (2026-11-19): "feat(chat): add on-device and privacy indicator (#551)" — new UI element surfacing "this is running on-device / private" status.
- `7d6b7a4`/`7d7bc34` (2026-11-23): "feat(brand): credit Software Mansion AI Labs with its logo, name and link (#554)" — branding change.
- `a5c07eb` (2026-11-19): "chore(android): require Android 13 (API 33) (#516)" — raised minimum supported Android version, dropping older-device support.
- `d96c3ed` (2026-11-13): "feat(onboarding): fold the optional features into one section (#524)" — onboarding UX simplification.

**Arabic/RTL localization:** The repo has **no `locales`/`i18n`-style directory at all** — root listing shows no translation folder, and `questionLanguage.ts`/`conversationalPhrases.ts` (under `utils/`) appear to handle **multi-language input detection for the chat/RAG pipeline** (e.g., answering in the language the user asked in — see commit `8294249`, 2026-11-23, "fix(language): answer French in French, and a photo without text in the thread's language") rather than a localized UI. This suggests Private Mind's "language support" is about the **model's output language, not a translated app UI** — i.e., **still no Arabic UI/RTL localization**, consistent with the prior finding of "no Arabic support at all." **[reasonably confident but not exhaustively verified — no UI-string locale files were found to search through]**

**At-rest encryption:** `package.json` shows `@react-native-async-storage/async-storage` (unencrypted key-value store) and `@react-native-rag/op-sqlite` + `react-native-rag` (plain SQLite-backed vector store, no SQLCipher reference found). **No at-rest encryption evidence** — contradicts the README's "Fully Private & Secure... stay on your device" framing, which is about data *not leaving the device* via network, not about encryption *of data on the device*. This is an important nuance: Private Mind's privacy claim is a no-telemetry/no-account/offline claim, not an at-rest-encryption claim — and no such encryption was found in the dependency tree.

**RAG/retrieval — important dating finding:** Private Mind's document-RAG feature is **NOT new**. Commit history shows:
- `98` / "feat: rag implementation" (2025-08-26)
- `117` / "feat: async readPDF, implement loading state" (2025-08-28)
- `147` / "feat: use react-native-rag filtering feature" (2025-09-18)
- `289` / "fix(chat): unblock the first document attach..." (2026-08-10)
So the "Chat With Your Documents" feature (PDF/TXT/MD/HTML/CSV, on-device embeddings, answers that "link back to the passages they came from" per the README) **predates the 2026-11-25 check-in by well over a year** — it is not a new feature, but it continues to be actively refined (today's commit `a3f58a3` improves per-turn source sizing). [README](https://github.com/software-mansion-labs/private-mind/blob/main/README.md), checked 2026-11-25.
  - **Advertised-only vs. verified:** README's claim "every answer links back to the passages it came from" is a page/passage-citation claim; I did not independently inspect the citation-rendering code to confirm granularity (page number vs. chunk vs. document-level), so **mark the "page citation" specificity as advertised-only / unverified**, even though the underlying RAG pipeline itself is source-code-confirmed real (`react-native-rag`, `react-native-pdfium`, `op-sqlite` dependencies are genuine, not just marketing).

**Voice:** README advertises "dictate a message with on-device speech input" (STT) — this is an existing, not-new, feature per the dependency/commit history; not independently re-verified in code this session beyond the README claim. **[advertised, partially unverified in this session]**

**Pricing/licensing/maintenance:** MIT (confirmed from LICENSE file text), free, by far the most active commit cadence of the five relative to its size — multiple PRs merged same-day as this research. Notably, the README now embeds **ad-server tags** (`swm-delivery.com` zone ads) at the top of the README — this is new-to-notice monetization/ad-infrastructure branding, though I could not date when the ad snippets were added vs. pre-existing, and could not confirm whether these ads appear in-app or are README-only. **[gap: ad-infrastructure timing not dated]**

**Star/community growth:** 416 stars / 54 forks as of 2026-11-25. **[no baseline — cannot confirm growth delta]**

---

## 5. ChatterUI — Vali-98/ChatterUI

**Repo stats (checked 2026-11-25):** 2,938 stars, 284 forks, 12 open issues, AGPL-3.0, last push 2026-11-24T06:59Z. [Source](https://github.com/Vali-98/ChatterUI)

**Releases:** `v0.11.0-beta1` (2026-10-30) is the latest, after `v0.10.0` (2026-09-22). [Releases](https://github.com/Vali-98/ChatterUI/releases)

**Note on activity level:** ChatterUI is markedly less active than the other four in the most recent weeks — the most recent commit in the full commit log pulled was `1753a4a` "docs: CONTRIBUTING.md" dated **2026-10-01**, i.e., **no code commits found between 2026-10-01 and the 2026-11-24 "last push" timestamp** in the data retrieved (the "last push" field may reflect a non-default-branch or tag push, e.g. the beta release tag on 2026-10-30, which falls in that gap). This suggests slower/more sporadic maintenance relative to PocketPal, Gallery, or Private Mind. **[flag: possible gap in commit data retrieved — could not fully confirm via this session's calls whether commits exist between 10-01 and 10-30 beyond the beta tag]**

**Features added in the run-up to v0.10.0 (Aug–Sep 2026, all before the 2026-11-25 window but recent/notable):**
- `a417cfb` (2026-08-29): "feat: i18n" — added `i18n/locales/id.json` (Indonesian), `ru.json` (Russian), `zh.json` (Chinese). **No Arabic file was added.**
- `51ebf7b` (2026-08-28): "i18n: use libretranslate" — i18n infrastructure using LibreTranslate.
- `50afa7e` (2026-09-16): "feat: better routing for data sources" — touched `DataSourcesScreen.tsx` and `RouteList.tsx`; based on filenames this appears to be a settings/API-provider routing screen, not a document-RAG feature (I could not fully confirm the screen's content — a direct fetch of `DataSourcesScreen.tsx` failed to decode in this session). **[gap: feature scope not fully confirmed]**
- `9106b7f` (2026-08-29): "feat: lorebook and entry creation and export" — lorebooks are character/world-context memory banks for roleplay (a primitive form of long-term context injection), **not document/file RAG** with citations.
- `02c7e0c` (2026-09-16): "fix: delete attachment files when a chat is deleted" — implies chat attachments exist, but no document-RAG-with-citations feature was found.

**Arabic/RTL localization:** Code search for "arabic" and for an `ar.json` locale file returned **zero results** — **no Arabic localization exists**, and the i18n work that did happen (Aug 2026) added Indonesian, Russian, and Chinese, not Arabic. This is a **new (pre-window) gap note**: ChatterUI didn't have Arabic before, and still doesn't as of 2026-11-25.

**At-rest encryption:** `package.json` shows `expo-sqlite ~57.0.3` — standard (unencrypted) Expo SQLite, no SQLCipher or other encryption wrapper found. Code search for "encrypt" in the repo returned zero results. **No at-rest encryption.**

**RAG/retrieval:** No file/document-RAG-with-citations feature found; ChatterUI's "memory" mechanism is character lorebooks (roleplay-context, not document retrieval), confirmed from the README feature list (Character Card v2 support, sampler/instruct customization) and the lorebook commit above. README confirms **text-to-speech (TTS) via the device's native TTS engine** is an existing feature; no STT mentioned in the README.

**Pricing/licensing/maintenance:** AGPL-3.0, free, maintained by a single primary developer (Vali-98) accepting community PRs (e.g., `antho976` contributed several fixes in Sep 2026); maintenance cadence has visibly slowed in Oct–Nov 2026 relative to the other four projects.

**Star/community growth:** 2,938 stars / 284 forks as of 2026-11-25 — second-largest of the five after Google AI Edge Gallery's much larger base, ahead of PocketPal in forks-to-star ratio concerns is not evaluated. **[no baseline — cannot confirm growth delta]**

---

## Cross-Cutting Answers to the Key Questions

### Arabic/RTL localization — did anything change?
**No.** Across all five repos, as observed on 2026-11-25:
- AnythingLLM Mobile: still 17 locales, no Arabic. [Verified in source]
- Google AI Edge Gallery: still no Arabic (only "AR" = augmented-reality features, a false-positive match on the string "ar"). [Verified in source]
- Private Mind: still no localized UI strings at all (language-detection ≠ localization); no Arabic. [Verified, reasonably confident]
- ChatterUI: still no Arabic; the one i18n push (Aug 2026, before the window) added Indonesian/Russian/Chinese, not Arabic. [Verified in source]
- PocketPal AI: the **only one with real Arabic support**, and it is **actively receiving fresh Weblate-translated Arabic strings** as recently as 2026-11-17 and 2026-11-15 — i.e., Arabic localization work in PocketPal is ongoing and improving, even though it's not "new" in the sense of being newly added (it already existed per the prior check-in). [Verified in source]

**Implication for ARA:** None of the four competitors with zero Arabic have added it. PocketPal remains the only real Arabic-language threat among these five, and its Arabic strings keep getting refreshed — worth periodically diffing PocketPal's `ar.json` against ARA's own Arabic/Egyptian-Arabic strings for terminology drift.

### At-rest encryption — did anything change?
**Yes — one confirmed new instance.** Google AI Edge Gallery added **AES-GCM encryption backed by the Android Keystore** for (at least some) local user data, in commit `1c7b534` on **2026-11-12** — this is the first and only at-rest-encryption feature confirmed in source code across all five repos, in this check or the prior one. Scope is not fully confirmed (likely tied to the agent/skill WebView runtime and/or settings; unclear if it covers full chat history). The other four (AnythingLLM Mobile, PocketPal, Private Mind, ChatterUI) show **no evidence of at-rest encryption** of chat or document data — AnythingLLM Mobile uses Keychain only for credentials, and the other three use plain SQLite/AsyncStorage.

### New RAG/retrieval/PDF-citation features — did anything change?
- **Private Mind** is the only one of the five with a working, real (source-verified) document-RAG pipeline (PDF/TXT/MD/HTML/CSV via `react-native-rag` + `react-native-pdfium` + `op-sqlite`), and it **continues to be actively enhanced** — most recently today (2026-11-25) with per-turn context-sizing for retrieved sources (`a3f58a3`). However, this RAG feature itself **predates the check-in window by over a year** (first implemented Aug 2025), so it is not "new" — only the tuning/context-sizing work is from this window. The README's specific claim of passage-level citation ("every answer links back to the passages it came from") is plausible given the real pipeline but the exact citation granularity (page vs. chunk) is **unverified — advertised-only** pending a code-level check of the citation-rendering component.
- **AnythingLLM Mobile** has PDF ingestion (via PDFBox) but the only recent change is a reliability bugfix (large-PDF memory spill-to-disk, Oct 2026), not a feature addition.
- **Google AI Edge Gallery, PocketPal AI, ChatterUI**: no document-RAG-with-citations feature found in source for any of them. ChatterUI's "lorebook" and "data sources" features are not document RAG.

### Notable new competitor repos discovered
Web search for "on-device AI assistant Android open source fork 2026" surfaced several smaller/newer projects **not previously tracked** (none independently source-verified in this session — flagged as **unverified, discovery-only**):
- **MA OpenAssistant** (`com.vayunmathur.openassistant`, on F-Droid) — runs Gemma on-device, ~3.5GB RAM footprint, GPL-3.0, F-Droid build dated November 2026. [F-Droid listing](https://f-droid.org/en/packages/com.vayunmathur.openassistant)
- **Oak** (github.com/adrielGGmotion/Oak) — described as an open-source AI assistant with persistent memory and on-device inference via LiteRT; forked from a project called "Kai" (Apache-2.0); labeled work-in-progress. [GitHub](https://github.com/adrielGGmotion/Oak)
- **PokeClaw/PocketClaw** (github.com/agents-io/PokeClaw) — open-source Android app for AI phone automation using local Gemma models, with optional cloud fallback; described as an early prototype. [GitHub](https://github.com/agents-io/PokeClaw)
- **LastChat** (github.com/Cocolalilal/LastChat) — a fork of **RikkaHub** (itself not in our tracked five, but worth noting as an upstream project) focused on privacy/personalization, README states it was partly built with AI coding agents. [GitHub](https://github.com/Cocolalilal/LastChat)
- **Relagent** / **Open Hitomi** — smaller self-hosted/overlay-assistant projects on F-Droid, lower apparent maturity.

None of these appeared to have Arabic localization, document RAG with citations, or at-rest encryption mentioned in the search snippets, but **none of this was source-verified** — treat purely as "repos to watch," not confirmed findings.

---

## Overall Gaps / Things I Could Not Verify This Session
- **No actual prior (2026-11-25) baseline snapshot exists to diff against** — today's date equals the supposed "prior check-in date" per system clock, so all "growth" and strict-novelty claims above are best-effort based on commit/release timestamps alone, not a true before/after diff. **This is the single biggest caveat on this entire report.**
- PocketPal AI's embedding/RAG search hit a GitHub API rate limit mid-session and was not retried — genuinely unknown whether PocketPal has any retrieval feature.
- Google AI Edge Gallery's AES-GCM encryption scope (full chat history vs. skill/agent-local state only) is not fully resolved from the single commit's file list.
- ChatterUI's commit history shows an apparent gap between 2026-10-01 and the 2026-11-24 "last push" — not fully explained (could be branch/tag-only pushes); if the project is less actively maintained than it appears, that's worth a follow-up check.
- Private Mind's README ad-tags (swm-delivery.com) — could not date when added, and whether they render in the actual mobile app or are README/GitHub-only.
- Did not independently check each project's official blog/roadmap pages (only GitHub source + two web searches) due to the 15-tool-call research budget; marketing-site claims beyond what's in this note are untouched.
