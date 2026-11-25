# ARA — الأمان والخصوصية

## المبادئ
1. الخصوصية افتراضيًا: المحادثات، النماذج، القياسات، الصلاحيات، الإعدادات في مساحة التطبيق الخاصة (`filesDir/ara`).
2. الإنترنت يُستخدم فقط لتنزيل نموذج يختاره المستخدم. لا تحليلات، لا رفع.
3. لا أسرار خادم داخل APK.

## المطبّق والمختبر
| الحماية | الدليل |
|---|---|
| الأخطاء لا تحمل محتوى المستخدم أو المسارات | `io_error_does_not_leak_message`، `missing_and_corrupt_files_fail_cleanly` |
| الكتالوج: https فقط، معرفات بلا `/` أو `..`، sha256 إلزامي للتثبيت | اختبارات registry + `bundled_catalog_is_valid_and_fully_verified` |
| التنزيل: تحقق SHA-256 قبل إعادة التسمية، حذف الملف التالف، قفل ضد التكرار، رفض http | 7 اختبارات model-manager بخادم HTTP محلي |
| المحادثات: معرفات آمنة ضد path traversal، كتابة ذرية، تجاهل الملف التالف | `rejects_path_traversal_and_survives_corruption` |
| الأدوات: schema صارم، precheck (https، هاتف، تاريخ)، صلاحيات، تأكيد إلزامي للأثر الخارجي، حد معدل، حد حلقات الوكيل (3) وحد الاستدعاءات (4/دور) | 26 اختبار ara-tools + 6 coordinator |
| مخرجات النموذج غير موثوقة: استدعاء مجهول/مشوه يُرفض، نتائج الأدوات تُعلَّم كبيانات لا تعليمات | `invalid_or_unknown_calls_are_denied`، `malformed_unterminated_and_excess_calls_are_rejected` |
| حقن تعليمات من محتوى ملف (مهما كان موديل مُخترَق يحاول تنفيذها، ومهما كانت الصلاحية "مسموح دائمًا"): صفر تنفيذ صامت عبر الخط الكامل (بحث حقيقي → `guard` → تأكيد) | `injected_document_content_cannot_trigger_a_silent_external_action` (7 هجمات) — T1.9 |
| Kotlin ينفذ فقط قائمة ثابتة من الأدوات، ويعيد فحص https | `HostActions.kt` |
| لا نسخ احتياطي سحابي/نقل أجهزة | `allowBackup=false` + `data_extraction_rules.xml` |
| لا إرسال خفي: الرسائل/المنشورات/المكالمات تفتح التطبيق الهدف والمستخدم يضغط إرسال | ADR-007 |
| مفاتيح API: Keystore AES-256-GCM، لا تُكتب في providers.json، لا تظهر في الأخطاء أو Debug | `api_hub_requires_consent_and_never_stores_keys`، `http_errors_are_mapped_without_leaking_the_key` |
| لا إرسال سحابي بدون موافقة صريحة لكل مزود؛ تغيير العنوان يلغيها | نفس الاختبار + `apiHubEnforcesConsent` (Kotlin) |
| درع الخصوصية يمنع خروج الهاتف/الإيميل/الكارت/الرقم القومي/المفاتيح | `streams_redacts_outgoing_and_restores_incoming` (يتحقق من الطلب الفعلي المرسل) |

## الأذونات في Android
- `INTERNET`: تنزيل النماذج، والمزودين السحابيين فقط بعد موافقة المستخدم.
- `SET_ALARM`: إذن عادي لـ Intents المنبه.
- لا أذونات حساسة (لا جهات اتصال، لا SMS مباشر، لا كاميرا، لا Accessibility).

## مخطط لاحقًا
- تشفير المحادثات بمفتاح من Android Keystore (لا نفترض أن Keystore يشفر الملفات تلقائيًا).
- مهارات الطرف الثالث: توقيع + مراجعة صلاحيات، وتعليماتها غير موثوقة ولا توسّع صلاحيات أي أداة.
- Supabase: RLS واختبارات عزل، التحقق من الاشتراك على الخادم فقط.
- Telegram Bot: التحقق من بيانات المصادقة على الخادم، تأكيد العمليات الحساسة.
