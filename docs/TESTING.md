# ARA — الاختبارات

## الأوامر
```bash
cd core
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace                       # سريع، بدون إنترنت أو نماذج

# اختبارات النموذج الحقيقي (اختيارية):
export ARA_HOME=/path/to/data                # يحتوي models/
cargo run --release -p ara-cli --bin ara -- models
cargo run --release -p ara-cli --bin ara -- download qwen2.5-1.5b-instruct-q4_k_m
ARA_TEST_MODEL=$ARA_HOME/models/qwen2.5-0.5b-instruct-q4_k_m.gguf \
  cargo test --release -p ara-inference --test llama_model
ARA_TEST_HOME=$ARA_HOME cargo test --release -p ara-ffi
cargo run --release -p ara-cli --bin ara -- agent qwen2.5-1.5b-instruct-q4_k_m "صحيني الساعة 6 ونص"

cd ../android
echo "sdk.dir=$HOME/android-sdk" > local.properties
./gradlew :app:testDebugUnitTest :app:assembleDebug
```

## آخر نتائج فعلية (2026-11-24، Linux x86_64)
| الوحدة | الاختبارات | النتيجة |
|---|---|---|
| ara-core-api | 5 | ok |
| ara-model-registry | 6 | ok |
| ara-device-profiler | 8 | ok |
| ara-model-selector | 9 | ok |
| ara-model-manager | 7 (خادم HTTP محلي: استئناف، تلف، إلغاء، حدود) | ok |
| ara-inference | 10 وحدات + 2 مع نموذج حقيقي (تشمل صحة KV cache) | ok |
| ara-tools | 26 | ok |
| ara-coordinator | 8 (نموذج مُبرمج، RAG، صلاحيات البحث) | ok |
| ara-search | 10 (عربي، أسطر، تكرار، حقن FTS، ثبات، ذاكرة) | ok |
| ara-ffi | 9 (منها E2E بنموذج حقيقي) | ok |
| Kotlin `RustBridgeTest` (JVM عبر الجسر) | 6 (يشمل AraCore والملفات والذاكرة) | انظر أدناه |

### سيناريوهات الوكيل بنموذج حقيقي (Qwen2.5-1.5B، `ara agent`)
| الطلب | النتيجة |
|---|---|
| احسبلي 15% من 2340 | calculator → 351 ✔ |
| حول 70 كيلو لرطل | 154.32 ✔ |
| ابعت لأحمد على الواتساب … +2010… | whatsapp_compose برقم ونص، ينتظر التأكيد ✔ |
| صحيني الساعة 6 ونص | set_alarm 06:30 ✔ (بعد تحسين الأوصاف) |
| حطلي تايمر 10 دقايق | set_timer 10 دقائق ✔ (بعد تحويل الوحدة إلى دقائق) |
| شغل الكشاف | flashlight on ✔ |
| Qwen2.5-0.5B لنفس المهام | فشل في معظمها ⇒ غير موصى به للمهارات |

### أسئلة على ملف عربي (`ara agent … --doc plan.md`، Qwen2.5-1.5B)
| السؤال | الإجابة |
|---|---|
| الميزانية الكلية كام؟ | ٨٥٠٠٠ جنيه (plan.md:8) ✔ |
| مين المسؤول عن التسويق؟ | منى سامي (plan.md:13) ✔ |
| امتى افتتاح المحل؟ | الخميس 12 ديسمبر 5 مساءً (plan.md:4) ✔ |
| الإيجار الشهري كام؟ | ١٢٠٠٠ جنيه (plan.md:9) ✔ |
| رقم تليفون المورد؟ (غير موجود) | «لم أجد» ✔ |

## لم يُختبر بعد
- أي تشغيل على هاتف/محاكي (لا KVM). يشمل: تنفيذ Intents، الأداء على ARM، onTrimMemory.
- جودة العربية بشكل منهجي (مجموعة تقييم مطلوبة).
- GPU/NPU.
