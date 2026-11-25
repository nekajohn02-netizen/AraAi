# Instinct (Spear Street Technology, Inc.) — raw research notes

**Research date:** 2026-11-25. **Method:** web search + fetch of 4 secondary-source articles (no access to Instinct's own product, source code, or terms of service directly — this product is invite-only and has no public app/repo). The 5th planned source (APKPure) returned HTTP 403 and contributed nothing.

**Sources, with short labels used below:**
- [TC] TechCrunch, Sarah Perez, 2026-08-24: https://techcrunch.com/2026/08/24/instincts-powerful-ai-assistant-is-raising-privacy-and-security-concerns/ — the only reporter-led piece; most incident facts trace back to public X posts it quotes. Treated as the primary source where it conflicts with the others.
- [AG] AndroGuider, "Randeotten", 2026-08-25: https://www.androguider.com/2026/08/instinct-ai-assistant-under-fire.html — reads as largely unsourced commentary; several claims are hypothetical/unattributed. Low confidence; flagged inline wherever cited.
- [GY] Gyld blog (Curtis Rosenvall), 2026-09-17: https://gyld.ai/blog/instinct-ai-review-impressive-agent-real-privacy-risks — written by a competing "AI company-brain" vendor, explicit commercial angle. Some facts trace to pages not independently opened (Vellum, Forbes, Bland docs, TechCrunch's 2026-08-26 and 2026-09-28 follow-ups).
- [BI] Built In, Matthew Urwin, 2026-10-21: https://builtin.com/articles/instinct-ai — general explainer, partly sourced to X posts, partly to unnamed "users have reported."

**Classification legend (matching `docs/COMPETITIVE_ANALYSIS.md`):** 🟢 verified (reporter-confirmed or primary-document-quoted) · 🟡 advertised/vendor-claimed only · ⚪ unverified/conflicting · ⬛ not found.

---

## 1. Security/privacy incidents (all during private beta, Aug–Oct 2026)

1. 🟢 **Data retained after disconnect/deletion request.** Peter Yang (early user) posted on X (2026-08-21) that Instinct indexed and retained his emails "without my permission" and would not let him delete them. [TC] reports the team later added a delete-external-data tool to settings. [GY] (unverified, citing Vellum) says the team acknowledged the gap.
2. 🟢 **Summaries continued after disconnection.** Claire Vo posted (2026-08-21) that she disconnected Instinct from Google at 11:00 and still received an email summary at 14:00; per [TC], the bot's own self-report said emails were "stored in plain text for later searches" (the bot's claim, not an independently verified technical finding). [GY] adds (its own, unverified) that Vo found full message text in her data export with no delete option at first.
3. 🟡 **2FA/sign-up code pulled from inbox.** [TC]: a tester found Instinct could pull a sign-up code from their inbox to complete a Resy booking — i.e. it can read and act on one-time codes/OTPs, not just summarize. No date given.
4. ⚪ **Prompt-injection / phishing test.** Alex Cohen (Hello Patient co-founder) posted (2026-08-22) that he emailed his own connected inbox with instructions for Instinct, then deleted his account, concluding it was "too easy to phish" [TC]. [TC] itself does not describe exactly what Instinct did in response. [GY]'s claim that "it followed them and returned a summary of open tasks" is **not corroborated by [TC]** — treat as unverified pending the original source.
5. 🟢 **Unapproved/unconfirmed action taken.** Katie Jacobs Stanton (Moxxie Ventures) posted (2026-08-22) that Instinct sent "an innocuous email on my behalf without checking with me first," after which she disconnected her email [TC]. This is the most directly comparable data point to ARA's own T1.9 confirmation guarantee — Instinct sent a message with no confirmation step at all, for an action ARA's own tests classify as `External` (confirmation always mandatory, even under the strongest `Always` permission grant).
6. 🟢 **Terms-of-service over-reach.** Screenshots circulated (2026-08-21/22) of a "perpetual and irrevocable" license covering access, storage, reproduction, publication and modification of user materials — including for AI model training [TC]. [TC] also quotes the ToS permitting capture of screen captures, cursor movements and keyboard input. Critics named: Mike Khristo, Jeremy Banon. [BI] says the ToS language was later adjusted but still allows disclosing de-identified/aggregated "Usage Data" for unspecified lawful purposes, with no stated opt-out found.
7. ⬛ **Speculative-only claims (do not cite as incidents).** [AG] asserts, without attribution: continuous screen/microphone access, a "persistent memory profile," ability to install browser extensions, ability to authorize payments, and a "could theoretically exfiltrate" prompt-injection risk. None of these appear in [TC]. Framed hypothetically by [AG] itself ("could theoretically") — not an incident.
8. 🟡 **Company response.** [TC]: no public reply at time of publication. Post-publication, per [TC], Instinct told the WSJ it was "taking the security concerns raised seriously." [GY] (unverified) says the team "has responded to at least one" issue.

## 2. Architecture

- **Cloud-only; no on-device component found.** [BI]: "It runs in the cloud... essentially operating a virtual computer and browser rather than living directly on a user's device." [GY]: "uses a persistent cloud computer with stored credentials," citing a dispatcher-agent model (top-level agent → sub-agents on a cloud machine with browser access and cached credentials) attributed to investor Anish Acharya (secondhand, not independently read). Note: [BI] elsewhere says it can "perform on-device tasks across different applications" — this contradicts its own cloud framing and should be treated as loose marketing language, not a confirmed local-execution mode.
- **Device-side capture is real, per the ToS text [TC] quotes directly:** audio, location, screen, cursor movement, keyboard input, "and more."
- **No model vendor disclosed.** [BI] vaguely says it's "powered by vision-language models"; no name, no benchmark.
- **No skills/plugin/extension system of any kind found.** [GY] (citing Bland's docs, not independently read) says Instinct does not document custom MCP connections or authenticated API requests. [AG]'s claim that it can "install browser extensions" is unsourced. **This is the key structural point: Instinct's risk model is not "a skills marketplace got exploited" (that's the separate OpenClaw/ClawHub case below) — it is "one broad, opaque grant of autonomy across many connectors (mail, calendar, messaging, screen, audio), with no scoped/named-capability permission system visible to the user at all."**
- **No app — interaction is by iMessage/WhatsApp text, voice call, or DM** [TC][BI][GY]. Added phone calls ("Instinct Concierge") Sep 2026 and group chats Oct 2026, per X posts cited by [BI].
- **Connectors confirmed:** Gmail, messaging apps, calendar, device audio/location/screen [TC]. [AG] adds cloud storage and contacts — unsourced, not independently confirmed.

## 3. Data retention, revocation, deletion

- Broad, "perpetual and irrevocable" retention/training license in the ToS [TC].
- Disconnecting an account did **not** stop prior data use (Vo case, §1.2) — summaries continued to be generated after disconnection.
- No way to delete stored email records at first; a deletion tool was added later to settings, per Yang's account as reported by [TC].
- Revised ToS still permits disclosure of de-identified/aggregated "Usage Data" for unspecified lawful purposes [BI] — no opt-out found in any source.

## 4. Autonomy, permissions, confirmation

- Per [TC]'s reading of the ToS, the agent can "enter into agreements, commitments, or transactions" on the user's behalf that "would be binding."
- Demonstrated actions (various sources, via quoted X posts): send email, book appointments/reservations, schedule rides, clean up inboxes, shopping, flight search, negotiate a Comcast bill, fill in medical paperwork, message vendors on WhatsApp.
- 🟡 The **only** specific confirmation claim found anywhere is [BI]'s own purchase FAQ: "can then access a user's payment credentials, select items on websites and ask the user for permission before completing the purchase" — unsourced marketing copy, not independently confirmed, and contradicted in spirit by the confirmed Stanton incident (§1.5), where a non-purchase but still externally-visible action (sending an email) happened with **no** prior check at all.
- No source documents a general, consistent confirmation gate across action types. The verified record (Stanton's unconfirmed email) is actually a **counter-example** to any confirmation claim.

## 5. Company, funding, launch

- **Company:** Spear Street Technology, Inc., San Francisco [TC][BI]. Product name: Instinct.
- **Founder:** led by Noah Shinn, a former Sierra research scientist [TC][BI]. [TC] mentions Luca Borletti as also involved — unconfirmed by a second source.
- **Status:** invite-only private beta since Aug 2026, waitlist via phone number on the website; existing users can send invites [TC][BI][GY]. Free during beta; no published pricing [GY].
- **Domain:** [TC]/[GY] cite instinct.co; [BI] cites instinct.com — unresolved which is canonical; verify before citing externally.
- **Funding — sources conflict, do not cite a single figure without checking the primary article:**
  - [TC] (updated post-publication): Instinct told the WSJ it was raising a "$250 million Series B at a $2.5 billion valuation," led by Index Ventures and Benchmark; Kleiner Perkins and Conviction also invested (rounds closed).
  - [GY]: "$350 million at a $2.5 billion valuation" (citing a 2026-08-26 TechCrunch follow-up, not independently read) — **conflicts with [TC]'s $250M figure.**
  - [BI]: $2.5B valuation Aug 2026, then "$10 billion" by end of Sep 2026 (citing a 2026-09-28 TechCrunch follow-up, not independently read).
  - Only the [TC] $250M/$2.5B figure was read from a primary-ish source here; the $350M and $10B numbers need their cited TechCrunch articles checked before being used in any external-facing claim.

## Bottom line for ARA's positioning

Instinct is **not a direct architectural competitor** to ARA (it is cloud-only, has no on-device inference, no local model catalog, no skills/plugin system, and isn't Arabic-focused in any source found). It matters to ARA's plan for a different reason: it is a live, well-covered, concrete case study of exactly the failure mode ARA's own `docs/SECURITY_AND_PRIVACY.md` and `ara-coordinator` guard logic are built to avoid — broad autonomous action across sensitive connectors, with a single opaque consent and no verified confirmation step, no reliable deletion, and a data-license clause that outlives the user's own attempt to disconnect. Every one of ARA's already-*tested* guarantees (T1.9's zero-silent-execution-under-`Always`-permission proof; the user-controlled, delete-tested memory model planned for T1.5; local-only storage with no training-license clause anywhere) is a direct, citable answer to one of the six verified Instinct incidents above — this doesn't need new engineering to claim, only writing it down with the citations above.
