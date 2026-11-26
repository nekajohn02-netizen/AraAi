# Ara — خارطة تنفيذ موحدة لتطوير مساعد أعمال ذكي

**آخر تحديث:** 2026-11-26  
**الحالة:** خطة مقترحة؛ لم يُراجع مستودع المشروع في هذه الجلسة للتأكد من أن أي بند غير الموجود قد نُفّذ بالفعل. راجع `IMPLEMENTATION_STATUS.md` وسجل القدرات قبل تحويل البنود إلى مهام تنفيذ.

> **الهدف:** تحويل الأفكار المتفرقة إلى خطة بناء مرتبة بالأولويات والاعتماديات ومعايير القبول. تطمح Ara إلى مساعد أعمال عربي يفهم سياق الشركة، ويحلل بياناتها، ويربط خدماتها، وينفذ أعمالًا مفوضة على حسابات المستخدم ضمن سياسات يحددها المستخدم.
>
> **حدود المنتج:** ربط حساب اجتماعي أو Google يتم بتفويض رسمي من المستخدم وعلى حساب يملك حق إدارته. يسمح النظام بأتمتة مسبقة الاعتماد داخل حدود محددة، لكنه لا يكتسب صلاحيات مفتوحة. الشراء والدفع وقبول العقود والحذف النهائي وتغيير إعدادات الأمان والأعمال عالية الأثر تحتاج تأكيدًا لحظيًا. لا وصول لأجهزة لا يملكها المستخدم، ولا اختراق تلقائي، ولا ادعاء أن Ara تستطيع ضمان نتائج البحث أو نمو الأعمال.
>
> **مستوى الإثبات:** `EXISTS_NEEDS_REVIEW` = وارد في وصف المشروع ويحتاج تأكيدًا من الكود والاختبارات؛ `NOT_STARTED` = مقترح لم يُثبت تنفيذه؛ `BLOCKED_BY_PROVIDER` = ينتظر API/مراجعة/حساب مطور؛ `VERIFIED` = نُفذ واختبر بمعيار قبول موثق. لا تُحوّل claim من صفحة تسويقية لمنافس إلى ميزة `VERIFIED` لدى Ara.

## 1. النتيجة المستهدفة ومعايير قبول المنتج

تقدم Ara مساحة عمل أعمال تجمع ملف الشركة، المعرفة والمهام، التسويق، الحسابات الاجتماعية، البريد، تحليلات الموقع/السوق، ومهارات ذكاء اصطناعي متخصصة. يستطيع المستخدم اختيار أتمتة بعض الأعمال بعد تكوين سياسة واضحة، ثم مراجعة سجل ما نفذته Ara وإيقافه أو سحب صلاحياته.

- [ ] يستطيع المستخدم إنشاء مساحة عمل وملف علامة تجارية، وإدارة منتجاته وعملائه ومشاريعه ومصادر معرفته دون خلط البيانات بين المساحات.
- [ ] يستطيع ربط حساب اجتماعي مهني/صفحة يديرها رسميًا عبر OAuth، ورؤية نطاق الصلاحيات، واختيار النشر التلقائي المحدود أو المراجعة اليدوية.
- [ ] يستطيع ربط Gmail بصلاحيات مستقلة للقراءة أو الإرسال، والاستفادة من الفرز والملخصات والمسودات، مع قواعد إرسال آلي ضيقة اختارها بنفسه.
- [ ] يستطيع ربط موقعه وSearch Console، والحصول على فحص تقني مدعوم بالأدلة، مع فصل فحص HTML عن بيانات Google المفهرسة وبيانات الأداء.
- [ ] لكل رقم أو استنتاج في تقرير أعمال/منافسين مصدر وفترة جمع وتعريف ودرجة ثقة؛ لا تُعرض المعرفة الداخلية أو تخمين النموذج كحقيقة موثقة.
- [ ] لكل مهمة مؤتمتة ميزانية/حدود/سياسة وإيقاف وسجل تشغيل وإيصال نتيجة، وتتعامل مع انقطاع المزود والتكرار والإلغاء.
- [ ] يستطيع المستخدم معرفة البيانات التي قُرئت أو أُرسلت لمزود نموذج، وإلغاء الاتصال وحذف البيانات المنقولة حسب سياسة واضحة قابلة للاختبار.
- [ ] لا تدّعي Ara أنها «أفضل من كل منافس» أو أنها تضمن الترتيب/الانتشار؛ تقيس حالات استخدام محددة على بيانات وتواريخ محددة.

## 2. قرارات بوابة P0 قبل كتابة تكاملات جديدة

هذه القرارات تمنع بناء واجهات لا يمكن تشغيلها فعليًا أو أمنًا.

- [ ] **P0-01 جرد المستودع:** اقرأ `docs/ROADMAP_TASKS.md` و`IMPLEMENTATION_STATUS.md` و`docs/MASTER_CAPABILITY_REGISTRY.md` و`docs/ARCHITECTURE.md` و`docs/EXPANSION_BLUEPRINT.md` وراجع `ara-tools`, `ara-smarthome`, `SkillsScreen.kt`, `SecurityScreen.kt`, `Missions` وAPI Hub. اربط كل بند هنا بملف فعلي أو دوّن أنه غير موجود.
- [ ] **P0-02 نمط التشغيل:** قرر local-only أو خدمة خلفية اختيارية أو هجين. تطبيق هاتف محلي لا يضمن جدولة مستمرة أو Webhooks أو مزامنة Gmail بعد إغلاقه. الجدولة الدائمة تحتاج عاملًا خلفيًا موثوقًا.
- [ ] **P0-03 نموذج المستخدم:** هل المنتج فردي أم فرق/شركات متعددة الأعضاء؟ إذا كانت مساحة عمل متعددة الأعضاء مطلوبة، صمم العزل والأدوار ومشاركة الأسرار قبل إضافة CRM أو تقويم مشترك.
- [ ] **P0-04 OAuth وملكية التطبيقات:** حدد مالك تطبيقات المطور وredirect URI وخطة اختبار/إنتاج ومراجعات Meta/LinkedIn/TikTok/Google. لا تستخدم أسرار تطبيقات موزعة داخل APK.
- [ ] **P0-05 البيانات والخصوصية:** حدد مناطق التخزين، الاحتفاظ، مزودي النماذج، حق حذف/تصدير، وما إذا كان يمكن اختيار inference محلي أو BYO API key.
- [ ] **P0-06 سجل الأدلة:** أضف حقول المصدر والتاريخ والحالة لكل قدرة داخل `MASTER_CAPABILITY_REGISTRY.md`، وسجل claims المنافسين في ملف منفصل.
- [ ] **P0-07 بوابة الأمن:** أنشئ نموذج تهديد لأسرار OAuth، Prompt Injection عبر البريد والويب، إساءة النشر، تسرب مساحة عمل، Webhooks مزورة، إعادة التنفيذ، وعمليات ماليّة غير مصرح بها.

### قرارات ADR المطلوبة

- [ ] `docs/ADR/ADR-021-social-and-email-automation.md`: OAuth، جدولة، آلية اعتماد السياسات، حدود الإرسال، ونموذج backend.
- [ ] `docs/ADR/ADR-022-workspaces-and-tenant-isolation.md`: الفرد مقابل الفريق، الصلاحيات، عزل البيانات والأسرار.
- [ ] `docs/ADR/ADR-023-ai-data-routing.md`: محلي/سحابي، مزودون، ميزانيات، ومنع إرسال بيانات حساسة دون اختيار واضح.
- [ ] `docs/ADR/ADR-024-seo-and-market-intelligence.md`: حدود الزحف والقراءة والاعتماد على Search Console ومصادر المنافسين.
- [ ] `docs/ADR/ADR-025-durable-agent-runtime.md`: إذا تقرر دعم مهام دائمة أو متصفح معزول أو Webhooks.

## 3. خارطة التنفيذ ومراحل الإصدار

### المرحلة 0 — جرد، نماذج، وبنية اختبار

- [ ] ربط الميزات الموجودة فعليًا بالـregistry وحالة تنفيذها؛ لا تكرر أو تستبدل الموجود قبل فحصه.
- [ ] تعريف نماذج البيانات وعقود الأحداث والـpermissions للمساحات والحسابات والمحتوى والتقارير والسياسات.
- [ ] إعداد mock connectors وfixtures منقحة عن البيانات الشخصية، واختبارات CI لا تستدعي منصات حقيقية.
- [ ] **بوابة الخروج:** ADRs مقبولة، نطاق MVP محدود، ومصفوفة OAuth/API/المراجعة مكتوبة.

### المرحلة 1 — قيمة أعمال محلية لا تعتمد على OAuth

- [ ] مساحة عمل وملف العلامة التجارية ومكتبة مستندات محلية.
- [ ] خطة عمل وCanvas وتسعير وهامش وربحية ونقطة تعادل مع المعادلات والافتراضات.
- [ ] مهام ومشاريع وإجراءات تشغيل وتقويم محتوى محلي ومسودات حملات.
- [ ] مهارات محتوى عربي وخدمة عملاء ومسودة المستثمرين وتحليل ملفات CSV/JSON.
- [ ] **بوابة الخروج:** كل قيمة مالية مرتبطة بمدخل ومصدر، ومحتوى النشر يبقى مسودة حتى اختيار المستخدم.

### المرحلة 2 — مساعد ذكاء موثّق

- [ ] استرجاع RAG من وثائق مساحة العمل مع citations وتاريخ صلاحية وإظهار نقص المعرفة.
- [ ] Model Router يراعي الخصوصية والكلفة والجودة، مع عدم تمرير بيانات حساسة للمزود افتراضيًا.
- [ ] AI evals للعربية والمصادر وPrompt Injection والتكلفة ودقة المهام.
- [ ] Business Pulse ورادار مهام/مخاطر مبني على المصادر التي ربطها المستخدم.
- [ ] **بوابة الخروج:** لا إجابة داخلية بلا مصدر أو تسمية واضحة للاستنتاج؛ لا تسرب بين المساحات.

### المرحلة 3 — حساب اجتماعي واحد end-to-end

- [ ] اختر المنصة الأولى بعد دراسة الطلب، نوع الحسابات المدعومة، access tier، المراجعة، الكلفة والـAPI.
- [ ] OAuth واختيار الحساب/الصفحة وعرض الصلاحيات وحفظ رموز آمن وفصل الاتصال.
- [ ] نشر يدوي مع معاينة وموافقة وتسجيل نتيجة، ثم لوحة بيانات أساسية.
- [ ] أضف نشرًا تلقائيًا فقط بعد اجتياز تدقيق التطبيق، مع سياسة محددة وسقف وتوقف.
- [ ] **بوابة الخروج:** اختبارات sandbox ومراجعة منصة وصلاحيات مسحوبة ومقاومة التكرار، وإيصال واضح للنشر.

### المرحلة 4 — SEO وGoogle Search Console

- [ ] مدقق تقني محلي لصفحات يحددها المستخدم، مع حدود زحف وسجل أدلة.
- [ ] OAuth لـSearch Console read-only، properties، Search Analytics وURL Inspection.
- [ ] صادرات توصيات/patch؛ تكامل CMS للكتابة يؤجل إلى ما بعد الموافقات والتراجع.
- [ ] **بوابة الخروج:** التقرير يميز بين HTML المرصود، بيانات نسخة Google المفهرسة، وبيانات الأداء؛ لا ضمانات ترتيب.

### المرحلة 5 — Gmail وعمليات البريد

- [ ] ربط Gmail تدريجيًا بأقل scopes: إرسال فقط، أو قراءة فقط، أو تعديل عند الحاجة؛ لا صلاحية شاملة افتراضيًا.
- [ ] فرز وتلخيص ومسودات؛ يبدأ الإرسال التلقائي في `draft_only` ثم سياسة ضيقة اختيارية بعد اختبارات.
- [ ] Gmail push/مزامنة خلفية فقط بعد بنية Cloud Pub/Sub/worker وخطة إيقاف وحذف واضحة.
- [ ] **بوابة الخروج:** Google OAuth verification إذا لزمت، سياسات Gmail، security assessment إن انطبق، وقياس Prompt Injection والإرسال الخاطئ.

### المرحلة 6 — محرك الوكلاء والأتمتة الدائمة

- [ ] Agent Registry وtool-grants وbudget وrun history وdurable workflows وkill switch.
- [ ] Browser sandbox لمواقع المستخدم ومهام الويب غير المغطاة بـAPI، من دون تجاوز CAPTCHA أو الحماية أو شروط الاستخدام.
- [ ] الوكلاء المنفصلون: باحث، محرر، محلل، بريد، SEO، خدمة عملاء؛ لكل وكيل أدواته المسموحة.
- [ ] **بوابة الخروج:** التوقف والاستئناف/idempotency والأذونات والميزانية تختبر تحت الانقطاع وإعادة التشغيل.

### المرحلة 7 — التوسع، الفرق، وميزات السوق المتقدمة

- [ ] إضافة منصات اجتماعية واحدة في كل مرة بعد مراجعتها، ثم تكاملات CRM/CMS/إعلانات/فواتير عند إثبات الطلب.
- [ ] مقارنة تنافسية مستمرة من مصادر محددة، مراقبة URLs بحدود مسموحة، وتجارب win/loss من مقابلات المستخدم.
- [ ] إعداد أدوار فريق وSSO ومراجعة compliance فقط إذا كان نموذج العمل يستدعي ذلك.
- [ ] **بوابة الخروج:** دعم وتشغيل وتكلفة ومراقبة وحذف بيانات قابلة للإثبات لكل connector جديد.

## 4. EPIC A — مساحة عمل وتشغيل شركة صغيرة

### مهام المنتج

- [ ] ملف مساحة عمل: الشركة، الصناعة، السوق، اللغة، المنطقة الزمنية، العملة، المنتجات والخدمات، الأهداف، الجمهور، عروض القيمة.
- [ ] Brand profile: نبرة، قواميس، كلمات ممنوعة، ادعاءات مسموحة، روابط صحيحة، عروض وأسعار ومواعيد صلاحيتها.
- [ ] مكتبة مصادر محلية: PDF/DOCX/CSV/JSON/صور، مع source owner وclassification وتاريخ التحديث.
- [ ] مشاريع ومهام ومواعيد واعتماديات وتقارير أسبوعية وسجل قرارات.
- [ ] CRM خفيف محلي اختياري: جهات اتصال، مراحل فرص، ملاحظات ومتابعة؛ دون scraping أو enrichment لأشخاص.
- [ ] استيراد/تصدير CSV/JSON بمعاينة ومنع تكرار وسجل تحويلات ومصدر البيانات.
- [ ] دعم عملاء: تصنيف محلي، مسودة رد، قاعدة إجابات مدعومة بمصادر، ومسار تصعيد للمشكلات الحساسة.
- [ ] استيراد فواتير/مصروفات/مبيعات للموجز المالي وتدفق نقدي تقديري، دون اعتبار المنتج محاسبًا أو مستشارًا ضريبيًا.
- [ ] business model canvas وخطة عمل وموجز إطلاق وموجز مستثمرين من حقائق موثقة وافتراضات موسومة.
- [ ] تذكيرات عبر Missions/WorkManager الموجود إذا تأكد وجوده؛ لا تنشئ scheduler موازيًا قبل مراجعة البنية الحالية.

### معايير القبول

- [ ] كل مساحة عمل منفصلة منطقيًا، وكل مصدر له مالك وحالة خصوصية.
- [ ] تظهر المعادلات والافتراضات في الحسابات المالية، ولا تُستنتج قيم غائبة.
- [ ] لا يخرج أي بريد/عرض/تقرير عن صيغة مسودة حتى يختار المستخدم مشاركة أو تصدير.

## 5. EPIC B — السوشيال والنشر المؤتمت

### المنتج والتقويم

- [ ] Social account registry يعرض المنصة والمالك والصفحات المرتبطة والصلاحيات وحالة token وحالة مراجعة التطبيق.
- [ ] Composer لإنتاج نسخة لكل منصة: نص، ميديا، الرابط، الجمهور، اللغة، النبرة، وقت النشر وقيود API.
- [ ] تقويم تحريري حسب الحملة والمنصة والحالة؛ إبطال موافقة أي نسخة بعد تعديلها.
- [ ] Media checks: صيغة/أبعاد/مدة/حجم ومصدر الترخيص، وروابط ملفات عامة فقط عند ضرورة API.
- [ ] Content QA: اتساق العلامة، مصدر الأسعار/العروض، الروابط، التكرار، الإفصاح عن إعلان/شراكة، أخطاء اللغة وحقوق الأصول.
- [ ] النشر البديل عبر Share Intent أو تصدير عندما API غير متاحة؛ لا يسمى هذا نشرًا تلقائيًا.

### OAuth والموصلات

- [ ] موصل مستقل لكل منصة، scope minimization، callback HTTPS، state ضد CSRF، PKCE حسب دعم المزود، refresh token آمن وإعادة تفويض عند الإبطال.
- [ ] مزودات مرحلة أولى مقترحة: Meta للصفحات وحسابات Instagram المهنية، ثم YouTube أو LinkedIn حسب جمهور Ara، وTikTok/X حسب مراجعة الوصول والكلفة.
- [ ] Meta: فصل Instagram Login عن Facebook Login، دعم حسابات مهنية فقط حسب API، مراجعة Advanced Access لخدمة المستخدمين، التعامل مع اختلاف metrics وحدود النشر [1] [2] [3].
- [ ] LinkedIn: Community Management access ومراجعة التطبيق وأدوار الصفحة، فصل إحصاءات العضوي عن المدفوع، ومتابعة إصدارات API [4] [5].
- [ ] X: OAuth 2.0 + PKCE، تكلفة/استهلاك، حد معدل، وفصل metrics العامة عن الخاصة/المدفوعة [6] [7].
- [ ] TikTok: Content Posting API، creator privacy options، مراجعة النشر العام، وقيود العملاء غير المدققة؛ لا وصف Display API المحدود بأنه analytics كامل [8] [9].
- [ ] YouTube: OAuth للمستخدم، حصة رفع خاصة، تدقيق امتثال لزيادة الحصة، وتأخر/حجب بعض analytics [10] [11].
- [ ] كل موصل يطبق response normalization مع إبقاء اسم metric الأصلي ومصدره.

### سياسات الأتمتة

- [ ] `AutomationPolicy` لكل حساب/حملة: مستوى استقلال، ما يُنشر، النبرة، الوسائط المعتمدة، الوقت/المنطقة، معدل النشر، ميزانية API، موضوعات ممنوعة.
- [ ] أنماط: `manual_review`, `publish_approved_queue`, `generate_and_publish_within_rules`. الأخير opt-in منفصل وليس default.
- [ ] Preview ومحاكاة قبل التفعيل، versioned policy، إبطال عند تغيير الحساب/الصلاحية/الحدود، إيقاف عند فشل متكرر أو محتوى حساس أو نقص بيانات.
- [ ] kill switch لمساحة العمل والمنصة والحملة؛ يوقف الطابور والعمال ولا يقبل scheduled jobs جديدة.
- [ ] كل عملية تحمل policy version وactor/source وaccount/content id ونتيجة API وتوقيت الموافقة أو التفويض.
- [ ] منع duplicate/idempotency، retries بحدود، backoff للـ429، cancellation، time-zone/DST، ومزامنة حالة النشر.
- [ ] نشر المحتوى السياسي/الصحي/المالي/القانوني/الادعاءات الحساسة يتطلب مراجعة مباشرة، حتى لو توجد سياسة نشر تلقائي عامة.

### قبول EPIC B

- [ ] فصل الحساب يوقف العمل فورًا، يمسح/يبطل token، ويوقف المهام المجدولة.
- [ ] timeout بعد احتمال نجاح POST لا ينتج نشرًا مكررًا.
- [ ] لوحة نشر تميز `queued/publishing/published/failed/cancelled` ولا تدّعي نجاحًا من مجرد إرسال الطلب.
- [ ] API unavailable أو app-review incomplete تعرض `BLOCKED_BY_PROVIDER` مع السبب والخطوة التالية.

## 6. EPIC C — Gmail ومساعد البريد

### درجات الوصول والوظائف

- [ ] موصل Gmail مستقل لـSearch Console. OAuth scopes منفصلة: `gmail.send` للإرسال، `gmail.readonly` للقراءة، `gmail.modify` للتعديل عند الحاجة. الوثائق تصنف بعض scopes حساسة وبعضها مقيدة؛ لا تطلب `mail.google.com` دون ضرورة الحذف النهائي [12] [13].
- [ ] شاشة تشرح الرسائل/البيانات التي ستُقرأ، ما سيُخزن، model provider، وفترة الاحتفاظ، مع ربط/فصل وإلغاء من Google.
- [ ] وظائف قراءة: inbox triage، daily brief، thread summary، استخراج موعد/وعد/سؤال، Gmail-to-task، وتقرير رسائل تحتاج متابعة.
- [ ] وظائف كتابة: draft reply، follow-up draft، support reply، رسائل investor update. ابدأ بحفظ draft ولا ترسل.
- [ ] تنظيم البريد: اقتراح labels، archive أو mark read؛ تمكين التنفيذ فقط بعد تحديد labels والقواعد.
- [ ] attachment assistant يعالج ملفات يختارها المستخدم، لا يفتح/ينفذ ملفات تنفيذية، ولا يرسلها لمزود نموذج بصمت.

### إرسال آلي مفوض

- [ ] Email policy تختار mailbox/labels/senders/recipient allowlist وموضوعات مسموحة، قالب، نافذة إرسال، حد رسائل/ساعة/يوم، وحالة fallback.
- [ ] ابدأ بـ`draft_only`، وبعد الاختبار يمكن للمستخدم تفعيل autoreply محدود لفئة دعم بسيطة بقالب معتمد.
- [ ] لا إرسال لجهة جديدة أو CC/BCC جماعي، ولا رسائل مالية/قانونية/صحية/خصوصية/توظيف أو شكاوى حساسة دون موافقة لكل رسالة.
- [ ] منع الرد على رسائل آرا نفسها، auto-response loops، تكرار thread، reply-all غير مقصود، أو إرسال مرفق/بيانات سرية.
- [ ] رسائل البريد والروابط والمرفقات محتوى untrusted لا يغير صلاحيات الأدوات أو تعليمات النظام. افصل استخراج المعلومات عن قرار الأداة.
- [ ] إيصال لكل إرسال مع policy version، recipients، subject، timestamp، result/thread id، ورابط الإيقاف.
- [ ] لا يمكن استرجاع رسالة أُرسلت بالفعل؛ اعرض ذلك عند تفعيل الإرسال المستقل.

### التشغيل والمزامنة

- [ ] مزامنة أولية محدودة بنطاق زمني وملصقات؛ لا تستورد كل الأرشيف افتراضيًا.
- [ ] بيانات Gmail في cache مشفر، per-user isolation، retention محدود، purge عند disconnect، وتقرير حذف يراعي النسخ الاحتياطية/الاستثناء القانوني.
- [ ] Gmail push يستخدم Cloud Pub/Sub وwatch وتجديدًا دوريًا؛ توثيق Google يطلب التجديد مرة على الأقل كل 7 أيام، ويذكر أن polling/sync أنسب لتحديثات أجهزة المستخدم؛ يلزم backend وrunbook [14].
- [ ] قبل نشر تكامل عام: OAuth verification، ومراجعة Google User Data policy؛ إذا نُقلت/خُزنت بيانات restricted scopes على server فقد يلزم security assessment [13].

### قبول EPIC C

- [ ] `send` وحده لا يقرأ البريد، و`readonly` وحده لا يرسل.
- [ ] فصل Gmail يوقف queue، يسحب tokens وwatch ويوقف مزامنة جديدة؛ purge state تتتبع copies والsummaries/embeddings.
- [ ] اختبارات prompt injection من رسالة أو مرفق لا تستطيع تغيير system/tool grant أو توليد إرسال خارج policy.

## 7. EPIC D — SEO وبيانات الموقع

### مدقق تقني محلي

- [ ] فحص URL يقدمه المستخدم: HTTP codes، redirect chain، robots.txt، sitemap، title/meta description، canonical، noindex، headings، links، alt text، mobile viewport، language، structured data، duplicate clues، HTTPS، Core Web Vitals إن توفرت قياسات.
- [ ] crawling محدود بالـdomain والعمق/العدد الذي يحدده المستخدم؛ احترام robots وrate، حد أقصى للملفات والحجم، إيقاف عند تحديات/حظر.
- [ ] التقرير يسجل exact evidence/selector/response/status، severity، confidence، impact caveat، suggested fix، timestamp، ونوع المصدر.
- [ ] اقتراح إصلاح HTML أو JSON-LD كـdiff قابل للتصدير؛ لا يكتب للموقع الحي دون CMS connector وموافقة منفصلة وbackup/undo.
- [ ] فحوص الأهلية الفنية لا تضمن أن Google سيزحف أو يفهرس أو يعرض الصفحة؛ robots.txt ليس وسيلة ضمان لإزالة URL [15] [16].

### Search Console

- [ ] OAuth read-only أولًا، property picker، تعريف URL prefix مقابل Domain property، وsiteUrl مطابق تمامًا [17].
- [ ] Search Analytics حسب query/page/country/device/date/search type مع توضيح أن API قد لا تعيد كل الصفوف [18].
- [ ] URL Inspection API لحالة النسخة المفهرسة؛ لا توصف بأنها اختبار حي للصفحة. URL inspection in Search Console browser tool له وظيفة مختلفة [19].
- [ ] cache/hit limits، queue، backoff و429، أولوية صفحات، ومراقبة quotas [20].
- [ ] sitemap submit/أي تعديل كتابة مؤجل ويحتاج scope وconfirmation واضحين [21].

### قبول EPIC D

- [ ] كل finding قابل لإعادة إنتاجه من الدليل المحفوظ.
- [ ] يميز التقرير «خطأ مرصود» من «احتمال/تحسين»، ولا يعطي score SEO موحدًا يوهم بدقة Google.
- [ ] الأرقام تظهر فترة البيانات والتوقيت والمنطقة/الفلاتر وآخر استرجاع.

## 8. EPIC E — الذكاء الاصطناعي وسياق الشركة

- [ ] `Knowledge Graph` خفيف يربط منتجات/عملاء/حملات/مهام/قرارات/صفحات ومصادرها.
- [ ] RAG هجين للنصوص المحلية: search + semantic retrieval، citations، source freshness، dedup، access filters، retrieval tests.
- [ ] صنف المعلومات: confirmed fact / assumption / model inference / user opinion. اعرض تعارض المصادر واطلب الحسم عند اختلاف سعر أو سياسة.
- [ ] `Business Pulse`: موجز أعمال أسبوعي من المهام والمبيعات/الملفات/الحملات والبريد المصرح به؛ يذكر source لكل insight.
- [ ] `Anomaly Watch`: تغيرات غير طبيعية في بيانات الزوار أو المنشورات أو الرسائل أو التكاملات، مع baseline وحجم عينة، دون ادعاء السببية.
- [ ] `Opportunity Radar` و`Next Best Action`: اقتراح فرصة وخطوة اختبار صغيرة، مرتبة بالأثر/الكلفة/الثقة/المخاطرة؛ لا تنفيذ تلقائي خارج policy.
- [ ] `Scenario Studio`: what-if للتسعير أو تواتر المحتوى أو ميزانية الحملة مع افتراضات وتوقع منفصل عن النتائج الحقيقية.
- [ ] `Product Launch Room`: خطة منتج، صفحة، بريد، محتوى، FAQ، دعم، مهام، وقياس إطلاق.
- [ ] `Customer Voice`: موضوعات متكررة من تعليقات/رسائل يملك المستخدم مصدرها، مع إخفاء البيانات الشخصية وتجميع عينات.
- [ ] `Content Repurposing`: تحويل محتوى يملكه المستخدم إلى منشورات ومقال ونشرة وموجز فيديو مع source/rights tracking.
- [ ] `Model Router`: محلي للقضايا البسيطة/الحساسة، ومزود أقوى بمفتاح المستخدم أو اختيار واضح للمهمات الصعبة؛ لا يُرفع محتوى إلى Cloud provider دون بيان.
- [ ] ميزانية لكل مهمة/وكيل لعدد الخطوات والطلبات والتكلفة والزمن، تتوقف عند الحد وتسمح برفعه يدويًا.
- [ ] AI eval suite للعربية/المصرية/RTL/المصادر/الهلوسة/التكلفة/Prompt Injection؛ لا تستخدم benchmark عام بدل تقييم وظيفي.
- [ ] `Plan → Preview → Execute → Verify`: الوكيل يشرح الخطة والأدوات والآثار والكلفة، ينفذ ضمن grants، ثم يتحقق من النتيجة من connector نفسه.

### قبول EPIC E

- [ ] لا يخلط الوكيل الإجابة المولدة بالحقيقة الموثقة؛ citations تطابق محتوى المستند الفعلي.
- [ ] لا يتدرب على محتوى العملاء من دون موافقة منفصلة صريحة.
- [ ] إخراج النموذج لا يمنح tool scopes؛ policy engine منفصل يقرر السماح.

## 9. EPIC F — تحليل تنافسي متقدم

### مصدر الأدلة والحوكمة

- [ ] سجل منافسين يدوي، URLs رسمية، الصناعة والسوق والجمهور، تاريخ آخر مراجعة.
- [ ] لكل evidence: URL/ملف، snippet أو لقطة مسموح بها، وقت الجمع، اللغة/البلد، طريقة الجمع، status، confidence، author/owner، expiry date.
- [ ] أنواع الدليل منفصلة: official product claim، third-party report، public social post، customer interview/Win-Loss، Ara inference.
- [ ] مراقبة URLs يختارها المستخدم أو BYO monitoring tool؛ domain allowlist، rate limit، robots/terms، عدم bypass login/paywall/CAPTCHA.
- [ ] إن ظهر bot detection أو unauthorized access، توقف التغطية وعلّم المصدر `INACCESSIBLE` بدل اختراع محتوى.

### ملفات تحليل المنافس

- [ ] `Competitor Profile`: الشريحة والقيمة والمنتجات والتسعير المعلن والتكاملات والتجربة والدعم والقنوات والرسائل.
- [ ] `Capability Matrix`: مقارنات بحالات استخدام ملموسة مع `verified/claimed/partial/unknown/not offered` وروابط الدليل.
- [ ] `Pricing Normalizer`: تكلفة لكل مستخدم/حساب/شهر/API operation، tiers، currency/tax، تاريخ العرض وpromo/conditions.
- [ ] `Customer Journey Benchmark`: discovery → signup → setup → first value → repeat use، يسجلها الباحث يدويًا ومن بيئة يحق له استعمالها.
- [ ] `Positioning Map`: محاور يختارها المستخدم، مع مبرر ومصادر للموضع؛ الرسم منظور تحليلي لا حقيقة موضوعية.
- [ ] `Message Map`: الوعود والجمهور والاعتراضات وCTA من صفحات عامة مقتبسة/مرتبطة.
- [ ] `Content Gap`: أسئلة/موضوعات يغطيها المنافس أو Ara من مصادر محددة، لا ادعاء keyword rank حي.
- [ ] `Share of Voice`: فقط لنطاق قنوات وكلمات وفترة معرّفة؛ لا يسمى حصة سوق فعلية.
- [ ] `Feature launch timeline`: تغييرات في الأسعار/الميزات/الشروط/التوظيف من أدلة مؤرخة مع تنبيه أن snapshot قديم.
- [ ] `Win/Loss`: أسباب اختيار/رفض المنتج من مقابلات موافق عليها، ترميز القضايا، وتوصيات اختبار.
- [ ] `Feature Prioritizer`: الأثر، التكرار، عدد العملاء، الكلفة، المخاطر، قيود المنصة، confidence، والتوافق مع هوية Ara.
- [ ] `Scenario response`: سيناريو تحرك منافس وخيارات استجابة بدائل، دون ردود/منشورات نيابة عن الشركة.
- [ ] `Moat tracker`: ثقة العملاء، زمن أول قيمة، عودة المستخدمين، دقة المهام، الكلفة، retention، جودة المعرفة الموثقة، وتغطية الموصلات المراجعة.

### قبول EPIC F

- [ ] التقرير يضم «ما لا نعرفه» وحدود العينة وتواريخ المراجعة.
- [ ] لا claim «تحليل شامل» إذا فُحصت صفحات مختارة فقط.
- [ ] كل توصية توصل إلى فرضية واختبار صغير ومالك ومقياس وتاريخ قرار؛ لا تقفز من تغير صفحة منافس إلى استنتاج استراتيجية مؤكدة.

## 10. تحليل Hanzo AI وInstinct: ما يؤثر في خارطة Ara

**تحديد المنتج:** Hanzo هنا هو `hanzo.ai`، وليس شركات أخرى تحمل Hanzo. صفحات المنتج الرسمية مصادر claims وليست تدقيقًا مستقلًا. Instinct هنا `instinct.com`.

### Hanzo AI — مستلهمات المنتج

- [ ] يعلن Hanzo عن منصة وكلاء/نماذج وتطبيق وCLI وMCP، وبيئات تطوير ووسائط متعددة [22].
- [ ] صفحة Agents تعرض agent profile يُعرّف بالنموذج والتعليمات والأدوات والميزانية، تشغيله من API/CLI/workflow/chat، سجل كل تشغيل، حد إنفاق، أدوات MCP حسب منح الصلاحية، computer sandbox، وتشغيل webhook/schedule [23].
- [ ] صفحة تسجيل agents تعلن التسجيل الذاتي بمحفظة، مفتاح API، usage budgets وrate limits وإضافة رصيد [24]. الفكرة الممكنة لAra: agent profiles وسقوف ميزانية، لا انشاء حسابات/مدفوعات ذاتية للوكلاء.
- [ ] الصفحة الرئيسية تسوق أرقام كلفة/سرعة/دقة؛ لا تعتمدها كمعيار تفوق قبل الحصول على benchmark مستقل قابل للإعادة [22].
- [ ] تنبيه due diligence: مركز Hanzo القانوني الذي فُحص في 2026-11-26 يصرح أن وثائقه **مسودات بانتظار counsel وغير نافذة**، رغم وجود نص privacy يشرح حالات استخدام البيانات؛ أعد التحقق من الوثيقة الملزمة وواجهة الموافقة قبل استخدام هذه الادعاءات كإشارة ثقة [25] [26].
- [ ] ميزات Ara المقترحة: Agent Registry، run history، per-task budget، tool-grant profiles، event-triggered durable workflow، browser sandbox، cross-device continuation، connector SDK، model routing.
- [ ] لا تنقل إلى Ara مزايا التسجيل التلقائي بالمحفظة أو الدفع بالعملات المستقرة ما لم يُبنَ business case وsecurity model مستقل؛ ليست متطلبًا لمساعد الأعمال.

### Instinct — مستلهمات المنتج والمخاطر

- [ ] الموقع الرسمي يصف مساعدًا شخصيًا عبر الرسائل أو المكالمات، يفهم ما يعمل عليه المستخدم ويمكنه استخدام الهاتف والكمبيوتر كما يفعل المستخدم، ويذكر أمثلة للحجز والتسوق والخدمات [27].
- [ ] تقرير TechCrunch خلال فترة الاختبار يذكر اتصالات بالبريد والرسائل والتقويم وإشارات شاشة/صوت/موقع، ومهام حجوزات وسفر وتسوق وتنظيم inbox ومتابعات؛ هذه أوصاف صحفية لمزاعم/تجارب لا ضمان توافر [28].
- [ ] تقرير سبتمبر يذكر عنوان بريد مستقلًا للمساعد للتواصل مع شركات أو إدارة التسجيلات والمتابعات وإضافة المساعد لسلاسل بريد، مع تصريحات عن 1Password والموقع والمدفوعات [29].
- [ ] تقرير أغسطس نقل مخاوف مختبرين من تخزين محتوى بريد بعد فصل الحساب، إرسال بريد دون اعتماد لحظي، وprompt injection عبر رسالة. نقل المقال أيضًا أن المستخدم أشار لاحقًا لإضافة أداة حذف؛ لا تعمم الحالة القديمة على كل إصدار حالي [28].
- [ ] شروط Instinct الحالية تقول إن المستخدم يظل مسؤولًا عن الأفعال، وتذكر الإشراف البشري المناسب والاستقلالية العالية، وتطلب تأكيدًا صريحًا قبل إتمام الشراء [30]. حول ذلك في Ara إلى سياسات تقنية قابلة للاختبار لا بند قانوني عام فقط.
- [ ] سياسة الخصوصية الحالية تقول إن فصل Google Workspace يوقف الاحتفاظ/الاستخدام لبياناته مع استثناء القانون، بينما خدمات أخرى قد تحتفظ ببيانات جُمعت سابقًا ضمن سجل المحادثة. وتقول إن بيانات Google Workspace لا تُستخدم لتدريب/تحسين نماذج، مع إعدادات وتفاصيل أخرى لباقي المحتوى [31]. صمم فصلًا ومحوًا لكل connector، لا زرًا واحدًا يوحي بحذف كل البيانات.

### ما تبنيه Ara بطريقة تميّزها

- [ ] واجهة مساعد واحدة مع Context Graph للأشخاص/المهام/المشاريع/الرسائل المصرح بها.
- [ ] Action Inbox يشرح الهدف والحساب والتكلفة والأثر والمستلمين، ويعرض preview قبل بدء العمل.
- [ ] Autonomy Dial: اقتراح، مسودة، موافقة فردية، أو تنفيذ آلي محدود؛ مستوى لكل نوع فعل ولكل حساب.
- [ ] Consent Ledger وسجل صلاحيات زمني، مع grant/revoke لكل مصدر، وحالة حذف/cache/embeddings.
- [ ] Browser session معزولة، عرض حي وsite allowlist؛ شراء/دفع/حذف/قبول شروط أو إرسال خارجي حساس يحتاج موافقة لحظية.
- [ ] Agent alias/mailbox منفصل اختياري، مع disclosure مناسب أنه مساعد AI، حدود إرساله، alias قابل للإيقاف، ومنع استلام رموز تحقق لحسابات ليست مفوضة.
- [ ] Location/audio/screen capture لا تعمل افتراضيًا؛ صلاحية وقتية عند الطلب، مؤشر مرئي، إمكانية تعطيل ومسح التاريخ.
- [ ] Action Receipt لكل فعل، Undo/Recovery حيث يمكن، وتوضيح ما لا يمكن التراجع عنه.
- [ ] Trust levels لا تزيد الصلاحيات تلقائيًا؛ الثقة تكسب من سجل نتائج لكن التفويض نفسه يظل ثابتًا حتى يغيره المستخدم.

## 11. EPIC G — مهارات الذكاء الاصطناعي المقترحة

- [ ] `content_strategy`: استراتيجية وتقويم وتوزيع وأهداف.
- [ ] `social_post_adaptation`: نسخ لكل منصة وحدودها والمعاينة.
- [ ] `brand_voice_guard`: فحص صوت العلامة والادعاءات والمصطلحات.
- [ ] `social_analytics_interpreter`: تفسير metrics مع definition/window/source.
- [ ] `seo_technical_audit` و`seo_content_brief`: أدلة وrecommendations لا وعود ترتيب.
- [ ] `business_model_canvas`, `pricing_and_break_even`, `cashflow_snapshot`: افتراضات ومعادلات ومصادر.
- [ ] `investor_update_draft`, `customer_reply_draft`, `email_followup`: مسودات أولًا.
- [ ] `business_pulse`, `operations_runbook`, `meeting_brief`, `decision_log`: استخدام Missions والذاكرة والمصادر.
- [ ] `competitor_profile`, `competitor_comparison`, `competitor_change_summary`, `win_loss_analysis`, `market_opportunity_scan`, `feature_prioritization`.
- [ ] `knowledge_research` و`evidence_based_brief`: citations وتاريخ صلاحية وconflict handling.
- [ ] `defensive_code_review`: مراجعة دفاعية لملفات يملكها المستخدم؛ لا استغلال أهداف خارجية.
- [ ] لكل skill metadata: id/name/trigger/input/output/tools/grants/data classes/limitations/examples/tests/version. فصل الـskill عن tool permission.

## 12. هيكل الملفات المقترح

الأسماء أدناه مقترحة؛ طابقها مع crate naming وهيكل المستودع بعد جرد P0.

```text
docs/
  BUSINESS_PRODUCT_SCOPE.md
  BUSINESS_INTEGRATIONS_MATRIX.md
  SOCIAL_AUTOMATION_POLICIES.md
  GMAIL_DATA_HANDLING.md
  SEO_AUDIT_RULES.md
  ANALYTICS_METRICS_DICTIONARY.md
  COMPETITOR_HANZO_INSTINCT.md
  COMPETITOR_CLAIMS_EVIDENCE.md
  AI_EVALUATION_METHOD.md
  RETENTION_AND_PURGE.md
  INCIDENT_RESPONSE_INTEGRATIONS.md
  ADR/ADR-021-social-and-email-automation.md
  ADR/ADR-022-workspaces-and-tenant-isolation.md
  ADR/ADR-023-ai-data-routing.md
  ADR/ADR-024-seo-and-market-intelligence.md
  ADR/ADR-025-durable-agent-runtime.md

core/crates/
  ara-business/src/{workspace.rs,brand_profile.rs,customer.rs,project.rs,approval.rs,audit_log.rs}
  ara-marketing/src/{campaign.rs,content_item.rs,content_variant.rs,editorial_calendar.rs,media_asset.rs}
  ara-social/src/{account.rs,capabilities.rs,permissions.rs,post.rs,metrics.rs,publishing_state.rs}
  ara-connectors/src/oauth.rs
  ara-connectors/src/social/meta/{oauth.rs,publishing.rs,insights.rs,webhooks.rs}
  ara-connectors/src/social/linkedin/{oauth.rs,publishing.rs,analytics.rs}
  ara-connectors/src/social/x/{oauth.rs,publishing.rs,metrics.rs}
  ara-connectors/src/social/tiktok/{oauth.rs,publishing.rs,display.rs}
  ara-connectors/src/social/youtube/{oauth.rs,upload.rs,analytics.rs}
  ara-connectors/src/google/gmail/{oauth.rs,messages.rs,drafts.rs,history_sync.rs,push_watch.rs}
  ara-connectors/src/google/search_console/{oauth.rs,search_analytics.rs,url_inspection.rs,sitemaps.rs}
  ara-automation/src/{policy.rs,policy_version.rs,trigger.rs,guardrail.rs,kill_switch.rs,run_log.rs,scheduler.rs,idempotency.rs}
  ara-agents/src/{registry.rs,run.rs,tool_grants.rs,budget.rs,verification.rs}
  ara-knowledge/src/{source.rs,document_version.rs,retrieval.rs,citation.rs,freshness.rs,access_scope.rs}
  ara-analytics/src/{metric_definition.rs,observation.rs,aggregation.rs,report.rs}
  ara-seo/src/{fetch.rs,crawler_policy.rs,html_checks.rs,robots.rs,sitemap.rs,canonical.rs,audit_finding.rs}
  ara-competitive-intel/src/{competitor.rs,evidence.rs,pricing_snapshot.rs,capability_matrix.rs,change_diff.rs,win_loss.rs,opportunity.rs}
  ara-tools/src/skills/ # فقط إذا كان تفكيك skills.rs مبررًا بعد الجرد

android/app/src/main/kotlin/ai/ara/app/
  business/WorkspaceScreen.kt
  marketing/ContentCalendarScreen.kt
  marketing/PostComposerScreen.kt
  marketing/ApprovalQueueScreen.kt
  integrations/ConnectedAccountsScreen.kt
  automation/AutomationCenterScreen.kt
  automation/AutomationPolicyEditor.kt
  automation/RunHistoryScreen.kt
  email/GmailConnectScreen.kt
  email/InboxBriefScreen.kt
  email/DraftReviewScreen.kt
  analytics/BusinessDashboardScreen.kt
  seo/SeoAuditScreen.kt
  competitive/CompetitorProfileScreen.kt
  competitive/ComparisonMatrixScreen.kt
  settings/PermissionsAndPrivacyScreen.kt

backend/ # فقط إن وافق ADR على backend
  oauth-callback/
  automation-orchestrator/
  publishing-worker/
  gmail-sync/
  pubsub-ingress/
  browser-sandbox/
  secret-vault/
  data-purge-worker/
  migrations/
  runbooks/
```

### ملفات واختبارات بجانب الكود

- [ ] fixtures منزوعة البيانات الحساسة لكل connector وإصدارات API.
- [ ] اختبارات OAuth/state/PKCE/expiry/revocation/scope insufficiency.
- [ ] اختبارات idempotency وrate limit وtimeout الغامض وretry وفصل الاتصال.
- [ ] HTML fixtures للـSEO، وmessage fixtures للبريد تتضمن Prompt Injection.
- [ ] schema contracts للـpolicy، action receipt، evidence، metrics، findings، agent run.
- [ ] runbooks لتسرب token، quota exhausted، connector outage، purge، توقيف publisher وPub/Sub.

## 13. الأمان والخصوصية والامتثال

- [ ] OAuth `state`, redirect URI allowlist، PKCE حيث يدعم، TLS، تشفير secrets، وAndroid Keystore محليًا. لا Client Secret داخل APK.
- [ ] فصل بيانات كل workspace/connector، عدم وضع tokens أو message bodies في logs/telemetry، ومنع cross-tenant retrieval.
- [ ] Treat email/web/content/media كمدخل غير موثوق؛ لا يستطيع المحتوى إعطاء agent صلاحيات أو تغيير system instructions.
- [ ] Webhook signature validation وreplay protection وidempotency وحدود الحجم والمعدل.
- [ ] consent screen تصف scopes وسببها والبيانات التي تقرأ/تكتب ومزودي المعالجة وفترة الاحتفاظ.
- [ ] Data export/delete/disconnect لكل connector، تفصيل النسخ المشتقة والنسخ الاحتياطية والمهل والاستثناء القانوني.
- [ ] لا تنفيذ شراء أو دفع أو قبول terms أو إرسال عقود/التزامات دون تأكيد لحظي؛ لا تخزين بيانات البطاقات.
- [ ] صفحة خصوصية سهلة: محلي/سحابي، model providers، استخدام التدريب، كيفية الإلغاء، الاحتفاظ والحذف.
- [ ] قبل الإطلاق: مراجعة قانونية/خصوصية، مراجعة OAuth المطلوبة، security review للـbackend، وخطة incident response.

## 14. الاختبارات والقياس والإطلاق

- [ ] اختبارات وحدات واتفاق API على fixtures؛ اختبارات end-to-end في sandboxes وحسابات اختبار تملكها Ara.
- [ ] OAuth tests: redirect mismatch، state مزور، رمز expired/revoked، scope ناقص، reconnect، account changed.
- [ ] Social tests: تعديل بعد الموافقة، تكرار، quota، media rejected، إلغاء نشر، وسياسة لا تغطي المنصة.
- [ ] Gmail tests: send-only/readonly separation، thread duplication، autoreply loop، المرفقات، prompt injection، purge.
- [ ] SEO tests: 3xx/4xx/5xx، noindex، robots blocked، canonical conflict، sitemap invalid، JS-only.
- [ ] Analytics tests: null/zero/partial/delayed/suppressed metrics، daylight saving، timezone وaggregation.
- [ ] Benchmark Ara لمهام منافسة محددة بنفس prompts/data/tier/date، مع تكلفة وزمن/دقة وتعديل بشري ونجاح action.
- [ ] لا إطلاق auto-publish/Gmail send قبل test flight محدود، kill switch، مراجعة السجلات، خطة استعادة وموافقة المنتج.
- [ ] مؤشرات الإصدار: activation, time-to-first-value, task completion, wrong-action rate, publish failure/duplicate, OAuth churn, cost/task, deletion completion, support ticket rate.

## 15. قرارات خارج النطاق حاليًا

- [ ] «آرا أقوى من كل منافس» أو ضمان سوق/SEO/انتشار؛ يستبدل بهدف قطاع أول ومقاييس تنافسية مستمرة.
- [ ] تشغيل شركة بالكامل من دون مستخدم، إدارة حساب بنكي أو تحويل مالي أو ميزانيات إعلانات ذاتيًا.
- [ ] إنشاء حسابات اجتماعية أو استخدام بيانات دخول/طرق غير رسمية أو تجاوز صلاحيات المنصات.
- [ ] التحكم بأي جهاز على الإنترنت، اكتشاف أجهزة عشوائية، أو قدرة remote access عامة. IoT يقتصر على أجهزة مقرونة صراحة في Home Assistant/MQTT وما يسمح به المستخدم.
- [ ] بناء crawler/فهرس ويب ضخم أو CRM/ERP/محاسبة/قانون/ضرائب كامل من الصفر قبل إثبات استخدام وحاجة.
- [ ] اختبارات اختراق هجومية تلقائية. يبقى الأمن دفاعيًا: الجهاز/المشروع الذي يملكه المستخدم، مراجعة الكود، قوائم تحقق OWASP، وتقارير إصلاح.

### استكمال نطاق المشروع الأصلي: IoT والأمن الدفاعي

- [ ] راجع `core/crates/ara-smarthome` وADR-020 و`SecurityScreen.kt` قبل أي تنفيذ جديد؛ حدّث القدرة الموجودة في registry بدل تكرارها.
- [ ] M4: عميل MQTT اختياري للـbroker الذي يملكه المستخدم أو يديره، مع TLS/mTLS حيث يدعمه، حفظ أسرار، allowlist للموضوعات والأجهزة، معالجة انقطاع واتصال.
- [ ] M5: سجل أجهزة مقترنة صريح؛ لا network scan لاكتشاف أجهزة لم يطلبها المستخدم. كل جهاز جديد يحتاج pairing/consent.
- [ ] M6: Matter عبر Home Assistant أولًا بدل بناء Matter stack مستقل؛ أعد تقييم الحاجة قبل إضافة بروتوكول آخر.
- [ ] M7: تنبيه واضح أن بعض الأجهزة قد تتصل بسحابة الشركة المصنّعة حتى عند استخدام تحكم محلي.
- [ ] كل أمر IoT يمر من confirmation gate الحالي للأثر الخارجي، ويظهر الجهاز/العملية/الحالة، مع deny-by-default للأوامر الحساسة.
- [ ] وسّع الوضع الأمني دفاعيًا: تحليل إعدادات جهاز المستخدم، صلاحيات التطبيقات وتحديثاتها، شرح مفاهيمي للثغرات، ومراجعة ملفات المشروع الذي يملكه المستخدم.
- [ ] أضف تقارير مراجعة كود محلية بقوائم OWASP مع دليل موضعي، شدة، ثقة، واقتراح إصلاح؛ لا exploit ولا scan عشوائي لأهداف خارجية.
- [ ] عيّن نموذجًا مناسبًا لمهام البرمجة/الأمن؛ اختبر API Hub بمزود حقيقي بمفتاح المستخدم قبل الادعاء بجودة قريبة من نموذج سحابي متقدم.

## 16. أول 23 تذكرة للتنفيذ

| ID | المهمة | الاعتماد | معيار الإنهاء |
|---|---|---|---|
| P0-01 | جرد التنفيذ والـADRs والقدرات القائمة | لا شيء | ربط كل ميزة بملف/اختبار وحالة موثقة |
| P0-02 | قرار backend/local + workspace model | P0-01 | ADR-021/022 مع أثر التشغيل والخصوصية |
| P0-03 | threat model وبيانات OAuth/الاحتفاظ | P0-02 | مخاطر وأدوات تخفيف واختبارات |
| P0-04 | schema للـWorkspace/Brand/Source | P0-01 | migrations/tests أو نماذج محلية |
| P1-05 | محرر محتوى وتقويم محلي | P0-04 | حفظ/تعديل/تصدير مسودات |
| P1-06 | Brand voice guard وحقائق مسندة | P1-05 | يرفض سعرًا/ادعاء بلا مصدر صالح |
| P1-07 | سياسة موافقات داخلية وسجل | P0-03 | تغيير المحتوى يبطل الموافقة |
| P1-08 | Agent Registry وRun History | P0-02 | تشغيل mock مسجل بمخرجات/كلفة |
| P1-09 | budget + kill switch | P1-08 | إيقاف فعلي تحت اختبار الاستئناف |
| P2-10 | مصفوفة اختيار منصة اجتماعية أولى | P0-01 | طلب/مراجعة/scopes/cost/risks موثقة |
| P2-11 | OAuth sandbox للمنصة الأولى | P2-10 | connect/revoke/expiry/scope tests |
| P2-12 | نشر يدوي بمنشور واحد | P2-11 | preview/approval/idempotency/receipt |
| P2-13 | لوحة metrics للمنصة الأولى | P2-11 | مصدر وتعريف وفترة/null handling |
| P2-14 | Auto-publish policy في shadow mode | P1-09,P2-12 | قرارات محاكاة لا تنشر خارجيًا |
| P3-15 | SEO crawler محدود وتقرير finding | P0-03 | fixtures/status/robots/evidence |
| P3-16 | Search Console read-only | P3-15 | property picker + query + quotas |
| P3-17 | Gmail scope/proof-of-concept review | P0-03 | Google OAuth requirements وقائمة scope |
| P3-18 | Gmail draft-only assistant | P3-17 | ملخص ومسودة، لا send endpoint |
| P4-19 | بريد/نشر آلي ضيق بسياسة مفوضة | P2-14,P3-18 | allowlist/limits/stop/audit tests |
| P4-20 | منافس matrix لـHanzo/Instinct + Ara baseline | P0-01 | claims/evidence/date/test results |
| P1-21 | سجل أجهزة IoT المقترنة صراحة | جرد `ara-smarthome` وADR-020 | لا اكتشاف تلقائي؛ pairing وconsent واختبارات صلاحية |
| P1-22 | عميل MQTT اختياري للـbroker الخاص بالمستخدم | P1-21 + مراجعة أمنية | TLS/secrets/topic allowlist/revoke/reconnect |
| P2-23 | مراجعة كود دفاعية على ملفات المستخدم | مراجعة أدوات `ara-tools` وسياسة الأمن | نتائج بمراجع ملفات، لا فحص أهداف خارجية |

## 17. معايير ترتيب الأعمال

رتّب كل بطاقة تنفيذ بحسب: قيمة مباشرة للمستخدم، اعتماد مزود/مراجعة، مخاطرة خصوصية/أمن، كلفة تشغيل، أثر على الاستدامة، وتعقيد الصيانة. لا تُسند نقاطًا رقمية شكلية بلا بيانات؛ اشرح مبرر الترتيب.

- [ ] أولوية P0/P1: أساس محلي، معرفة موثقة، صلاحيات، سجل، إيقاف، نموذج تكاليف.
- [ ] أولوية P2: OAuth اجتماعي واحد، نشر يدوي، metrics حقيقية.
- [ ] أولوية P3: SEO/Search Console وGmail draft-only.
- [ ] أولوية P4: أتمتة بريد/نشر سياسة-محدودة بعد أدلة جودة ومراجعة مزود.
- [ ] أولوية P5: browser sandbox، monitoring تنافسي، فريق/SSO، ربط CRM/CMS، ودعم Matter عبر Home Assistant بعد قياس الحاجة.
- [ ] أولوية P6: توسيع المنصات أو معاملات الأعمال فقط بعد اختبار جدوى، صلاحيات، امتثال، ودعم تشغيلي.

## 18. مراجعة إضافية من `pasted_content_2.txt`: الاستوديو والجهاز والبيت الذكي

هذا الملحق مصدره وصف المشروع المرفق، وليس فحصًا مباشرًا للكود. بعض الحالات فيه معلنة على أنها مكتملة ومختبرة؛ تُعامل كـ`EXISTS_REPORTED_BY_PROJECT_DOC` إلى أن يراجع P0-01 الملفات والـcommit والاختبارات. لا نعيد بناء القدرة نفسها قبل حسم حالتها.

### 18.1 تصحيحات حالة التنفيذ التي يجب التحقق منها أولًا

- [ ] **O1 سياسة المحتوى:** المستند يقول إن `core/crates/ara-tools/src/content_policy.rs` موجود، ومعه أداة `generate_image` في `ToolRegistry` تستدعي الحظر قبل الصلاحية/الموديل، مع اختبارات. تحقق من الملفات والـcommit `5d591a3` والاختبارات المذكورة، ثم حدّث `IMPLEMENTATION_STATUS.md` بدل تسجيلها كبند جديد.
- [ ] **توصيل الصورة:** المستند يذكر `ara-cloud::image_gen` بصيغة BYO-key ككود موجود لكنه معطّل بقرار منع التكلفة الخارجية. تحقق أن مسار التوليد لا يستدعى من الواجهة ولا يستهلك شبكة/مفتاحًا تلقائيًا، وأبقِ تفعيل المزود مقفولًا حتى يُحسم قرار التكلفة.
- [ ] **M1 Home Assistant:** المستند يقول إن `core/crates/ara-smarthome` وعميل REST و5 اختبارات موجودة، لكن الربط بـ`ToolRegistry` وبوابة ADR-006 وواجهة Kotlin وتخزين token في Keystore غير مكتمل. تحقق من كل طبقة على حدة؛ لا تعتبر وجود crate مساويًا لميزة عاملة للمستخدم.
- [ ] **NPU profiler:** المستند يقول إن `classify_image_gen_tier()` يقدر وجود NPU من اسم الشريحة فقط. وثّقه `heuristic/غير مثبت benchmark` ولا تعرضه كقياس فعلي لقدرة NPU.
- [ ] **Missions/WorkManager وcrosswalk وMedia3 وHardware APIs:** راجع وجودها وعقودها قبل الاعتماد عليها في تعليم اللغة أو استوديو الوسائط أو أعمال طويلة.
- [ ] **قرار محتوى المنتج:** افحص أن سياسة المشروع المرفقة بشأن منع توليد الموسيقى والمحتوى الإباحي/الجنسي ما زالت قرار المنتج الحالي؛ إذا كانت سارية، سجّلها في policy registry ومتطلبات المنتج، لا في prompt النموذج فقط.

### 18.2 إضافات قابلة للبناء: الاستوديو الإبداعي على الهاتف

- [ ] **محرر صور K1:** قص/تدوير/إضاءة/فلاتر ونص واستيكر وتصدير؛ اختر نطاقًا عمليًا على أندرويد ولا تصفه بأنه Photoshop أو محرر طبقات احترافي.
- [ ] **مونتاج K2:** ابنِ على Media3 Transformer بعد جرد الموجود؛ ابدأ بقص وتجميع ومسارات محدودة وانتقال ونص وLUT بسيط، ثم اختبر على أجهزة حقيقية.
- [ ] **عارض 3D K3:** استيراد وعرض وتدوير/قياس/تعليقات لنماذج glTF أولًا؛ لا تسوّقه كنمذجة أو sculpting أو rigging أو بديل Blender.
- [ ] **اختبارات استوديو:** قياس وقت التصدير والحرارة والذاكرة والبطارية على فئات أجهزة، إظهار الحجم/الدقة/الوقت المتوقع، إلغاء آمن، عدم إبقاء موديل صور وموديل لغة كبيرين محملين معًا.
- [ ] **مهام ثقيلة:** لا تعد بمهام هاتف تستمر ساعات بأداء مضمون؛ الأفضل تفويضها لاحقًا إلى جهاز Companion اختياري، مع تقدم وإشعار وحدود حرارة/بطارية واضحة.

### 18.3 التوافق المحلي: Local API وAra Companion

- [ ] **L1 Local API:** قيّم Ktor Server أو NanoHTTPD (أو خيارًا مناسبًا محدثًا) قبل التنفيذ؛ لا تكتب HTTP parser يدويًا. ابدأ بـlocalhost وواجهات قراءة فقط، token عشوائي قابل للتدوير، زر إيقاف مرئي، ورفض افتراضي للطلبات من الشبكة المحلية.
- [ ] **L2 Companion:** أضف pairing صريح بكود يظهر على الهاتف والكمبيوتر، قناة مشفرة، هويات أجهزة وسحب الاقتران. نطاق الخدمة: إرسال مهمة محددة واستلام نتيجة، لا تحكم عام بسطح المكتب.
- [ ] **L3 مهام طويلة:** اختبر Foreground Service مع نوع الخدمة الصحيح وإشعار دائم وقيود Android/OEM/Doze، وقدّم تقديرًا واقعيًا وإرشاد الشحن؛ لا ضمان إكمال.
- [ ] **L4 تكامل التطبيقات:** افحص تنفيذ Android Sharesheet القائم، وقيّم دور Default Assistant App وشريط اختيار النص كتكاملات رسمية اختيارية. لا تستخدم Accessibility Service أو overlays أو VPN أو صلاحيات عامة بديلًا سريعًا.
- [ ] اختبر Local API على شبكة Wi-Fi عدائية: CSRF/DNS rebinding بحسب التصميم، تسريب token، replay، طلبات من تطبيقات أخرى، حدود payload، rate limiting، وإيقاف الخادم عند قفل المستخدم أو انتهاء الجلسة.

### 18.4 البيت الذكي وIoT

- [ ] **M3 بوابة فعل حقيقي:** كل `call_service` يمر عبر confirmation gate في النواة قبل الوصول إلى Home Assistant؛ اختبر أن «مسموح دائمًا» لا يتجاوز سياسة الحماية إن كانت ADR-006 تشترط ذلك.
- [ ] أضف شاشة Kotlin لحالة الربط، ping، الأجهزة/الكيانات المصرح بها، معاينة الأمر، ونتيجة التنفيذ.
- [ ] خزّن Home Assistant long-lived token في Keystore/مخزن أسرار مناسب، مع إخفائه عن logs وإمكان الإبطال.
- [ ] M4 MQTT: ابدأ فقط بعد نجاح M1، واتصل بـbroker يحدده المستخدم، مع TLS/auth، allowlist للمواضيع، عدم الاشتراك في wildcard افتراضيًا، وعدم استكشاف الشبكة.
- [ ] M5 Device Registry: كل جهاز يضاف يدويًا أو عبر استعلام مسموح صريح من Home Assistant؛ سجل من أقرنه ومتى، وأزل صلاحياته عند فك الربط.
- [ ] M6 Matter: استفد من دعم Home Assistant أولًا؛ لا تنفذ stack خاصًا قبل قرار منفصل واختبار حاجة.
- [ ] M7 تحذير للمستخدم أن بعض الأجهزة قد تتصل بسحابة المصنع حتى مع تحكم محلي.

### 18.5 تعليم اللغة وتوليد الوسائط وسياسة التكلفة

- [ ] **P Language Teacher:** مهارة منفصلة للمفردات والقواعد والاختبارات والتكرار المتباعد، وإعادة استخدام `core/lexicon/ar_crosswalk.json` إن ثبتت صلاحيته. لا تكرر وضع «المعلم» العام.
- [ ] تمارين النطق معلقة على قرار STT/TTS معماري (المستند يذكر T1.6)، مع تقييم الخصوصية/التكلفة محليًا أو مزودًا؛ لا تعرض نطقًا مصححًا قبل اختبار دقة عربية فعلية.
- [ ] **Q Image Generation محلي:** قبل بناء الاستدلال، benchmark على أجهزة حقيقية وتحقق من الرخصة مباشرة من المصدر الرسمي، الحجم، RAM، وقت/حرارة، وجود NPU فعلي، وإمكانية تنزيل/حذف النموذج. تقديرات المستند عن ثوانٍ/جهاز محدد تبقى فرضية حتى القياس.
- [ ] فرض قاعدة تحميل واحدة: إفراغ نموذج اللغة قبل تحميل نموذج الصور على الأجهزة المحدودة، ثم إعادة تحميله بعد انتهاء المهمة؛ اختبر الذاكرة والإغلاق القسري.
- [ ] **Q Video Generation:** أبقِها `CLOSED_BY_PRODUCT_COST_CONSTRAINT` إذا ظل قرار «لا تكلفة خارجية» قائمًا. لا تستخدم خدمة مدفوعة أو مفتاح المستخدم كحل التفافي عن القرار.
- [ ] **قرار التكلفة الذي ينقص الخطة:** اكتب policy موحدًا يفرّق بين (أ) كلفة تدفعها Ara، (ب) كلفة مزود مدفوعة بمفتاح المستخدم، (ج) API مجاني محدود، (د) استضافة/خادم/تخزين/رسائل؛ حتى OAuth أو API «مجاني» قد يسبب كلفة تشغيلية غير مباشرة. الوضع الافتراضي: لا رسوم أو إنفاق دون إظهار الجهة والسقف وموافقة، ولا مسار BYO-key يتفعل تلقائيًا.
- [ ] أي مزود صورة/فيديو/نموذج خارجي يحتاج `feature flag` مغلقًا، سقف إنفاق، اختبار محتوى وسياسة O1 قبل الإرسال، وقياس ما يخرج من الجهاز.

### 18.6 ترتيب هذا المسار وملفات التنفيذ

- [ ] تحقق O1 أولًا لأنه بوابة سلوك للمخرجات المولدة؛ اختبره قبل كل مسار image generation محلي أو سحابي، عبر Rust وواجهة Kotlin، لا عبر prompt فقط.
- [ ] أكمل M1: ToolRegistry + confirmation + Keystore + Kotlin UI قبل M4/MQTT.
- [ ] قيّم L4 (ShareSheet/Default Assistant/text selection) كتحسين أندرويد قليل التبعية قبل تطوير Companion كامل.
- [ ] نفذ K1/K2 بنطاق إنتاجي محدود بعد اختبار Media3/حرارة؛ K3 أقل أولوية ويحتاج مكتبة 3D منفصلة.
- [ ] L1 ثم L2 فقط بعد تقييم خادم HTTP محلي وأمن pairing. لا تسجل L3 كضمان للمهام الطويلة.
- [ ] P1/P2/P3 تعليم نصي أولًا؛ أوقف P4 النطق إلى أن يُحسم T1.6.
- [ ] Q1/Q2 توليد صورة محلي اختياري بعد feasibility gate، لا وعد على كل الأجهزة؛ لا فيديو عند بقاء قيد التكلفة.
- [ ] أضف مسارات مرشحة: `android/.../studio/`, `android/.../companion/`, `core/crates/ara-companion/`, `core/crates/ara-media/`, `core/crates/ara-media-gen/` (feature-gated), `core/crates/ara-smarthome/`, `core/crates/ara-tools/src/content_policy.rs`, و`core/crates/ara-tools/src/skills/language_teacher.rs`؛ لا تنشئ مسارًا موجودًا بالفعل.

## 19. المصادر الرسمية ومصادر التغطية المستخدمة

المراجع الخارجية التالية تؤيد حدود/قدرات API أو تنقل claims منتجات كما هو موضح بجانبها. راجع الوثائق مجددًا عند تنفيذ الموصل لأن النطاقات والمراجعات والحدود تتغير.

### Meta

[1]: https://developers.facebook.com/documentation/instagram-platform/ "Instagram Platform"
[2]: https://developers.facebook.com/documentation/instagram-platform/content-publishing "Instagram Content Publishing"
[3]: https://developers.facebook.com/documentation/instagram-platform/insights "Instagram Insights"

### LinkedIn، X، TikTok، YouTube

[4]: https://learn.microsoft.com/en-us/linkedin/marketing/community-management/community-management-overview?view=li-lms-2026-11 "LinkedIn Community Management API"
[5]: https://learn.microsoft.com/en-us/linkedin/marketing/community-management-app-review?view=li-lms-2026-11 "LinkedIn Community Management App Review"
[6]: https://docs.x.com/fundamentals/authentication/oauth-2-0/authorization-code "X OAuth 2.0"
[7]: https://docs.x.com/x-api/getting-started/pricing "X API Pricing"
[8]: https://developers.tiktok.com/doc/content-posting-api-reference-direct-post "TikTok Direct Post API"
[9]: https://developers.tiktok.com/doc/content-sharing-guidelines "TikTok Content Sharing Guidelines"
[10]: https://developers.google.com/youtube/v3/guides/quota_and_compliance_audits "YouTube Quota and Compliance Audits"
[11]: https://developers.google.com/youtube/analytics/reference/reports/query "YouTube Analytics reports.query"

### Gmail وGoogle Search Console وSEO

[12]: https://developers.google.com/workspace/gmail/api/auth/scopes "Gmail API scopes"
[13]: https://developers.google.com/identity/protocols/oauth2/production-readiness/restricted-scope-verification "Google restricted-scope verification"
[14]: https://developers.google.com/workspace/gmail/api/guides/push "Gmail API push notifications"
[15]: https://developers.google.com/search/docs/essentials/technical "Google technical requirements"
[16]: https://developers.google.com/search/docs/crawling-indexing/robots/intro "Google robots.txt guide"
[17]: https://developers.google.com/webmaster-tools/v1/sites/list "Search Console sites.list"
[18]: https://developers.google.com/webmaster-tools/v1/searchanalytics/query "Search Analytics query"
[19]: https://developers.google.com/webmaster-tools/v1/urlInspection.index/inspect "URL Inspection API"
[20]: https://developers.google.com/webmaster-tools/limits "Search Console API limits"
[21]: https://developers.google.com/webmaster-tools/v1/sitemaps/submit "Search Console submit sitemap"

### Hanzo AI وInstinct

[22]: https://hanzo.ai/ "Hanzo AI product home"
[23]: https://hanzo.ai/solutions/agents "Hanzo AI agents solution"
[24]: https://hanzo.ai/agents "Hanzo for agents"
[25]: https://hanzo.ai/legal "Hanzo Legal and Trust Center"
[26]: https://hanzo.ai/legal/privacy "Hanzo Privacy Policy"
[27]: https://instinct.com/ "Instinct official product page"
[28]: https://techcrunch.com/2026/08/24/instincts-powerful-ai-assistant-is-raising-privacy-and-security-concerns/ "TechCrunch on Instinct privacy and security concerns"
[29]: https://techcrunch.com/2026/09/09/viral-ai-assistant-instinct-now-has-its-own-email-address/ "TechCrunch on Instinct's assistant email"
[30]: https://instinct.com/terms "Instinct Terms of Service"
[31]: https://instinct.com/privacy-policy "Instinct Privacy Policy"
