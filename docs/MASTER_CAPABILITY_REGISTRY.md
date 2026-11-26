# Ara — Master Capability Registry

**Date:** 2026-11-25
**Purpose:** one cross-referenced table answering "is X real" across every
domain this project's various prompts have raised — the original 70-feature
blueprint (Categories A–J), the pre-existing T-roadmap
(`docs/ROADMAP_TASKS.md` T0–T4), `docs/EXPANSION_BLUEPRINT.md` (K/L/M/N),
and the 93-domain catalog (A–CO) from the latest "unified master build"
prompt. This is additive, not a replacement — it points at the existing
detailed docs rather than repeating their content, per that prompt's own
rule 4: "If an equivalent document already exists, update it or reference
it rather than creating duplicate documentation."

**Status vocabulary** (exact wording from the prompt that requested it):
`EXISTING_AND_VERIFIED` · `EXISTING_BUT_UNVERIFIED` · `PARTIALLY_IMPLEMENTED`
· `PLANNED` · `MISSING` · `BLOCKED_BY_HARDWARE` · `BLOCKED_BY_EXTERNAL_SERVICE`
· `NOT_APPLICABLE`.

`NOT_APPLICABLE` here means: realistically out of scope for what Ara
actually is (a local-first, Arabic-first personal Android assistant) —
not "nobody thought of it." Marking dozens of exotic domains (CAD, CNC,
astronomy, bioinformatics, industrial telemetry...) as roadmap items this
project will "get to eventually" would be exactly the kind of inflated,
unfalsifiable scope this registry exists to prevent.

| Domain (from the 93-item catalog) | Status | Evidence / reason |
|---|---|---|
| A. OS intelligence and device control | PARTIALLY_IMPLEMENTED | `HostActions.kt` (fixed allow-list of Intents), `ara-device-profiler` (RAM/CPU/thermal). No generic "device control" framework — intentional, see ADR-006. |
| B. Android platform integration | EXISTING_AND_VERIFIED | The whole app: Compose UI, CameraX not yet used, WorkManager (Missions), Keystore (KeyVault), notifications, splash screen. |
| C. Windows desktop integration | MISSING | Planned as "Ara Companion" (`EXPANSION_BLUEPRINT.md` L2) — no code yet, library decision not made. |
| D. Linux desktop integration | MISSING | Same as C — one companion protocol, not per-OS. |
| E. macOS desktop integration | MISSING | Same as C. |
| F. Browser automation | MISSING | Not planned — conflicts with "no Accessibility/overlay automation" (original blueprint §6). |
| G. Computer vision / screen understanding | MISSING | No ML Kit vision integration exists yet (blueprint C15–C19 unbuilt). |
| H. OCR and document layout | PARTIALLY_IMPLEMENTED | PDF text-layer extraction + page citation is real and tested (ADR-014). **No OCR of scanned/image documents** — explicitly rejected, not silently missing (T1.1). |
| I. Speech recognition | PARTIALLY_IMPLEMENTED | `Voice.kt::VoiceInput` wraps Android's own `SpeechRecognizer` (zero-cost, no bundled model, no cloud) — real, built, `./gradlew :app:assembleDebug` confirmed (2026-11-26). Not a bundled Whisper engine (T1.6 blueprint C21's original ask); quality/offline-ness depend on the device's own engine. Never run on a real device. |
| J. Text-to-speech | PARTIALLY_IMPLEMENTED | `Voice.kt::VoiceOutput` wraps Android's own `TextToSpeech` (zero-cost, on-device). Real, built, same verification as I. Never run on a real device. |
| K. Audio processing | MISSING | — |
| L. Video processing | MISSING | Media3 Transformer planned (original blueprint Category F); not built. |
| M. Timeline/media project automation | MISSING | — |
| N. Image editing and graphic design | MISSING | Planned (`EXPANSION_BLUEPRINT.md` K1 = editing existing images, explicitly not "Photoshop"). **Generation from scratch (text-to-image/video) is a separate, newer item: `EXPANSION_BLUEPRINT.md` Category Q, 2026-11-26** — on-device generation judged 🟡 (narrow device floor, needs NPU) to ❌ (video), with the only realistic high-quality path being a BYO-cloud-key extension of the existing API Hub pattern. |
| O. Vector graphics and typography | MISSING | — |
| P. 3D scene creation | MISSING | Planned as a **viewer**, not a modeling tool (K3). |
| Q. CAD and parametric modeling | NOT_APPLICABLE | Needs desktop-class precision input + GPU; not a phone-app fit. |
| R. PCB and electronics design | NOT_APPLICABLE | Same reasoning as Q. |
| S. 3D printing preparation | NOT_APPLICABLE | — |
| T. CNC/G-code workflows | NOT_APPLICABLE | — |
| U. Robotics simulation | NOT_APPLICABLE | — |
| V. Scientific computation | NOT_APPLICABLE | Out of product identity (personal/Arabic assistant, not a sci-compute platform). |
| W. Statistical analysis | NOT_APPLICABLE | — |
| X. Research and literature management | PARTIALLY_IMPLEMENTED | A `research` persona exists using `web_search`/`search_documents`/`calculator` — but **Ara does not actually browse the web yet** (own docs say so plainly, `ROADMAP_TASKS.md` item 10). |
| Y. GIS and mapping | PARTIALLY_IMPLEMENTED | `maps_search` opens a `geo:` URI in the user's map app. No GIS processing. |
| Z. Astronomy / space science | NOT_APPLICABLE | — |
| AA. Environmental monitoring | NOT_APPLICABLE | — |
| AB. Agriculture / precision farming | NOT_APPLICABLE | — |
| AC. Renewable energy monitoring | NOT_APPLICABLE | — |
| AD. IoT device integration | PARTIALLY_IMPLEMENTED | `core/crates/ara-smarthome` (Home Assistant client) — tested, **not wired to ToolRegistry/Kotlin yet** (ADR-020). |
| AE. Smart-home orchestration | PARTIALLY_IMPLEMENTED | Same as AD. |
| AF. Building management | NOT_APPLICABLE | — |
| AG. Industrial telemetry | NOT_APPLICABLE | — |
| AH. Network diagnostics | MISSING | — |
| AI. Server operations | NOT_APPLICABLE | **No production server exists for this product** — this is a GitHub Codespaces dev container (no DirectAdmin, no web/db server process running), confirmed directly and summarized in `PROJECT_OVERVIEW.md`. |
| AJ. Cloud and infrastructure management | NOT_APPLICABLE | Same as AI. |
| AK. Container/virtualization management | NOT_APPLICABLE | — |
| AL. Database intelligence | EXISTING_AND_VERIFIED (small scale) | SQLite+FTS5 `index.db` is real and tested. "Slow-query review" etc. don't apply — there's no server DB. |
| AM. API laboratory | MISSING | — |
| AN. Software development workspace | MISSING | Chat can discuss code generically; no dedicated dev workspace. |
| AO. Code security and quality (as a product feature) | NOT_APPLICABLE | Not part of product identity. |
| AP. CI/CD and release engineering | MISSING | **Checked, not assumed:** no `.github/` workflow files exist in this repo. |
| AQ. Dependency/supply-chain security | PARTIALLY_IMPLEMENTED | `Cargo.lock` pinned, model catalog restricted to official publishers (ADR-004). No automated CVE scanning. |
| AR. File intelligence | EXISTING_AND_VERIFIED | Import/search/delete documents, `FilesScreen.kt`, tested in Rust. |
| AS. Knowledge graph | MISSING | Explicitly deferred (T1.5 "Memory Graph" is `⬜` in `ROADMAP_TASKS.md`); today's memory is a flat list by design. |
| AT. Semantic and hybrid search | PARTIALLY_IMPLEMENTED | Lexical FTS5 + Arabic crosswalk is real (T1.2a ✅). Embedding-based semantic search (T1.2b) is `⬜`, not started. |
| AU. Personal memory | EXISTING_AND_VERIFIED | User-controlled memory list, tested. |
| AV. Project memory | MISSING | No workspace/project isolation concept exists. |
| AW. Long-running task engine | PARTIALLY_IMPLEMENTED | Missions (WorkManager, approvals, audit log) is real and tested in Rust (T0.3) — but it's one-step-at-a-time re-enqueueing, not a general checkpoint/DAG task engine. |
| AX. Workflow designer | MISSING | — |
| AY. Event and trigger engine | MISSING | T2.1 Automation is `⬜`. |
| AZ. Scheduler and recurring jobs | PARTIALLY_IMPLEMENTED | `MissionWorker` re-enqueues itself; no user-facing "recurring schedule" UI. |
| BA. Multi-device coordination | MISSING | — |
| BB. Remote worker management | MISSING | Planned (`EXPANSION_BLUEPRINT.md` L2/L3). |
| BC. Communication connectors | PARTIALLY_IMPLEMENTED | `HostActions` opens WhatsApp/Telegram/SMS/Email with content pre-filled — **user presses send**, no API-based auto-send (ADR-007, deliberate). |
| BD. Email and calendar | PARTIALLY_IMPLEMENTED | Intent-based compose only; no OAuth read/write access. |
| BE. Messaging integrations | PARTIALLY_IMPLEMENTED | Same as BC. |
| BF. Team collaboration | NOT_APPLICABLE | Single-user personal assistant by design. |
| BG. Personal productivity | PARTIALLY_IMPLEMENTED | Alarm/timer/calendar-event intents exist; no task/habit tracker. |
| BH. Learning and tutoring | NOT_APPLICABLE | Not a currently planned persona. |
| BI. Translation and localization | PARTIALLY_IMPLEMENTED | The app itself is ar/en localized. ML Kit on-device translation (a *feature*, original blueprint F5) is unbuilt. |
| BJ. Publishing and editorial | NOT_APPLICABLE | — |
| BK. Business research | PARTIALLY_IMPLEMENTED | Same `research` persona as X, same web-browsing caveat. |
| BL. SEO and web analytics | NOT_APPLICABLE | — |
| BM. Digital commerce operations | NOT_APPLICABLE | — |
| BN. Inventory and supply chain | NOT_APPLICABLE | — |
| BO. Finance and expense organization | NOT_APPLICABLE | — |
| BP. Legal-document organization | NOT_APPLICABLE | — |
| BQ. Accessibility assistance | PARTIALLY_IMPLEMENTED | Reduce-motion + TalkBack live-regions added this session; full TalkBack/font-scale-200%/RTL matrix never run on a real device. |
| BR. Digital archiving and preservation | MISSING | — |
| BS. Backup and recovery | PARTIALLY_IMPLEMENTED | `allowBackup=false` (a *privacy* decision, excludes Ara from Android Auto Backup) exists; `core/crates/ara-backup` (2026-11-26, ADR-021) adds a tested passphrase-encrypted export/import archive — still no Kotlin UI (SAF file picker, passphrase screen) calling it. |
| BT. Observability and telemetry | PARTIALLY_IMPLEMENTED | The Privacy Center activity log (ADR-019) is a form of this, metadata-only. No crash/performance telemetry pipeline. |
| BU. Quality assurance automation | PARTIALLY_IMPLEMENTED | 197 Rust unit tests passing. Zero on-device/Android instrumentation tests. |
| BV. Model evaluation and benchmarking | EXISTING_AND_VERIFIED | `ara verify`, Arabic benchmark (104 questions, 12/13 models measured), retrieval-eval harness — all real, see `docs/ARABIC_BENCHMARK.md`/`RETRIEVAL_EVAL.md`. |
| BW. Agent evaluation and simulation | PARTIALLY_IMPLEMENTED | The 7-attack prompt-injection harness (T1.9) is a real instance of this; no general agent-simulation framework. |
| BX. Extension marketplace | MISSING | **Explicitly deferred by product-owner decision** (`ROADMAP_TASKS.md` §1: "لا متجر مهارات الآن") — not an oversight. |
| BY. Plugin/connector registry | PARTIALLY_IMPLEMENTED | `ToolRegistry` + Skills system (trust tiers, import, revocation) is real. No generic external-service "connector" system beyond API Hub. |
| BZ. Identity and trust management | PARTIALLY_IMPLEMENTED | Skill trust tiers (ADR-017) exist. **No user account/identity system at all** — the product is intentionally account-less. |
| CA. Policy and permission engine | EXISTING_AND_VERIFIED | `ara-tools` dispatch gate (ADR-006), proven against 7 prompt-injection attacks with zero silent execution (T1.9). |
| CB. Artifact provenance | PARTIALLY_IMPLEMENTED | Activity log tracks tool/outcome; no file/artifact version provenance system. |
| CC. Reproducible builds and workflows | PARTIALLY_IMPLEMENTED | Pinned `Cargo.lock`/toolchain versions. No CI to automatically prove reproducibility. |
| CD. Configuration/version management | PARTIALLY_IMPLEMENTED | `settings.json` versioned loosely via `#[serde(default)]`; no formal schema-migration system. |
| CE. Incident management | NOT_APPLICABLE | No production service to have incidents. |
| CF. Predictive maintenance | NOT_APPLICABLE | — |
| CG. Digital twin / simulation orchestration | NOT_APPLICABLE | — |
| CH. Personal cloud workspace | MISSING | Encrypted cloud outbox (T2.5) is `⬜`, not built. |
| CI. Self-hosted deployment management | NOT_APPLICABLE | No deployable backend exists to manage. |
| CJ. Resource and cost optimization | PARTIALLY_IMPLEMENTED | Storage cap, context-token budget, device-tiered model selection are real. No "cost" dimension (no paid backend). |
| CK. Universal search and clipboard | PARTIALLY_IMPLEMENTED | In-app document/conversation search; `copy_to_clipboard` tool exists. No OS-wide universal search. |
| CL. Knowledge capture | EXISTING_AND_VERIFIED | `import_document`/`add_memory`, tested. |
| CM. Application capability discovery | NOT_APPLICABLE | `HostActions` uses a fixed allow-list by design (ADR-006) — generic "scan installed apps" was deliberately not built, not forgotten. |
| CN. Compatibility negotiation | NOT_APPLICABLE | Only matters for a multi-engine/multi-OS platform, which doesn't exist yet. |
| CO. Integration health monitoring | PARTIALLY_IMPLEMENTED | API Hub "test connection" and Home Assistant `ping()` exist per-integration; no unified dashboard. |

## Reading this table

- **EXISTING_AND_VERIFIED** entries have real, passing tests cited in
  `docs/ROADMAP_TASKS.md`/`docs/ARCHITECTURE.md` — check those files for the
  exact test names, not just this summary.
- **PARTIALLY_IMPLEMENTED** is the largest bucket by far. That's accurate
  for an early-stage app with a disproportionately mature Rust core relative
  to its Kotlin UI surface (see `IMPLEMENTATION_STATUS.md`'s fourth pass).
- **NOT_APPLICABLE** (≈30 of 93 domains) is a judgment call about product
  fit, not a technical impossibility claim — if the project owner actually
  wants Ara to do CAD or bioinformatics, that's a product-direction
  conversation, not something this registry should quietly assume away or
  quietly assume into scope.

## Where the rest of that prompt's requested documents already live

Rather than create 16 more files with largely overlapping content:

| Requested document | Where it already lives |
|---|---|
| `MASTER_PLAN.md`, `ARCHITECTURE.md` | Already exist at `docs/`, updated this session (ADR-020). |
| `SECURITY_AND_PRIVACY.md`, `TESTING.md` | Already exist at `docs/` from before this session. |
| `EXISTING_VS_PROPOSED_MATRIX.md`, `NEW_CAPABILITIES_CATALOG.md`, `CAPABILITY_COVERAGE.md` | This file. |
| `EXPANSION_IMPLEMENTATION_PLAN.md`, `EXPANSION_DEPENDENCY_GRAPH.md`, `TECHNOLOGY_SELECTION_MATRIX.md` | `docs/EXPANSION_BLUEPRINT.md`'s per-category tables and "what to build first" section. |
| `EXPANDED_COMPETITOR_RESEARCH.md` | `docs/COMPETITIVE_ANALYSIS.md` + `docs/COMPETITIVE_BENCHMARKS.md` (already code-verified against real competitor repos, not marketing pages). |
| Autonomous-business/marketing/investor-relations/social-media/market-monitoring/coding/cybersecurity capability gap analysis + skills-folder plan | `docs/AGENTIC_BUSINESS_AND_IOT_BLUEPRINT.md` (2026-11-26) — also the explicit scoping-down of "control any internet-connected device" to the user's own paired devices only. |
| `NEW_MODULE_REGISTRY.md` | `docs/ARCHITECTURE.md`'s crate list. |
| `EXPANSION_RISK_REGISTER.md`, `EXPANSION_DECISIONS.md` | The ADR sequence in `docs/ARCHITECTURE.md` already records decisions + the risk reasoning for each. |
| `EXPANSION_TEST_REPORT.md` | `IMPLEMENTATION_STATUS.md` (this session's actual `cargo test`/`gradlew` runs, with real pass counts — the separate `PERFORMANCE_REPORT.md`/`ROOT_CAUSE_REPORT.md`/`SECURITY_REPORT.md` files that used to duplicate parts of this were removed 2026-11-26, their content already being inside `IMPLEMENTATION_STATUS.md`). |
| `INTEGRATION_OPPORTUNITIES.md` | `docs/EXPANSION_BLUEPRINT.md`. |
| `BEYOND_LIMITS_AUDIT.md` / `BEYOND_LIMITS_FINAL_REPORT.md` | Not created — "beyond limits" framing conflicts with this project's own anti-inflation principle; the honest version of that content is this registry plus `EXPANSION_BLUEPRINT.md`'s explicit ❌ verdicts. |

If any one of these would genuinely be clearer as its own file, say which
one and it'll be split out — better than 18 thin files duplicating each
other on day one.
