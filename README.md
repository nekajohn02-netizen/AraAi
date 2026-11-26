# ARA

مساعد ذكاء اصطناعي محلي أولًا لـ Android: واجهة Kotlin/Compose ونواة Rust.

جديد على المشروع؟ ابدأ بـ [PROJECT_OVERVIEW.md](PROJECT_OVERVIEW.md) (شرح المشروع والمراحل في صفحة واحدة).

- `core/` — نواة Rust (Cargo workspace).
- `docs/` — الخطة والبنية والأمان والاختبارات. التفصيل الحي في [docs/MASTER_PLAN.md](docs/MASTER_PLAN.md).

```bash
cd core && cargo test
```

- `android/` — تطبيق Kotlin/Compose. `cd android && ./gradlew :app:testDebugUnitTest` (يتطلب Android SDK + NDK 29.0.14206865 و`cargo-ndk`).
