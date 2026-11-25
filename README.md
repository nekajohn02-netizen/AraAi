# ARA

مساعد ذكاء اصطناعي محلي أولًا لـ Android: واجهة Kotlin/Compose ونواة Rust.

- `core/` — نواة Rust (Cargo workspace).
- `docs/` — الخطة والبنية والأمان والاختبارات. ابدأ بـ [docs/MASTER_PLAN.md](docs/MASTER_PLAN.md).

```bash
cd core && cargo test
```

- `android/` — تطبيق Kotlin/Compose. `cd android && ./gradlew :app:testDebugUnitTest` (يتطلب Android SDK + NDK 29.0.14206865 و`cargo-ndk`).
