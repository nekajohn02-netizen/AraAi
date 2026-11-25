//! `ara` developer CLI. Exercises the real core end to end on any Linux
//! machine (including an Android device via adb shell).
//!
//!   ara profile
//!   ara models  [--mode automatic|quality|speed] [--lang ar] [--ctx 4096]
//!   ara download <id>
//!   ara bench    <id> [--ctx 2048] [--tokens 64]
//!   ara chat     <id> <message> [--max 256] [--cancel-after N] [--system TEXT]
//!   ara verify   <id>   (measures tool use, JSON output, Arabic, long context; saves verification.json)
//!   ara bench-ar <id> [--limit N] [--failures]   (Arabic Benchmark v1, saves arabic_bench.json)
//!   ara agent    <id> <message> [--doc FILE]... [--persona ID]  (skills + tools + specialist mode)
//!
//! Data dir: $ARA_HOME or ~/.local/share/ara (models/ and benchmarks.json).

use ara_core_api::{AraError, AraResult, CancellationToken};
use ara_device_profiler::{DeviceProfile, HostHints};
use ara_inference::llama::LlamaEngine;
use ara_inference::{ChatMessage, GenerationParams, InferenceEngine, LoadParams, Role};
use ara_model_manager::{InstallState, ModelStore, StoragePolicy};
use ara_model_registry::{Capability, ModelRegistry, ModelSpec};
use ara_model_selector::{Benchmark, ModelSelector, SelectionMode, SelectionRequest};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

const CATALOG: &str = include_str!("../../../catalog/models.json");

struct Ctx {
    home: PathBuf,
    registry: ModelRegistry,
    store: ModelStore,
}

impl Ctx {
    fn new() -> AraResult<Self> {
        let home = std::env::var_os("ARA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share/ara")))
            .ok_or_else(|| AraError::InvalidInput("set ARA_HOME".into()))?;
        let store = ModelStore::open(home.join("models"))?;
        Ok(Self {
            registry: ModelRegistry::from_json(CATALOG)?,
            store,
            home,
        })
    }
    fn spec(&self, id: &str) -> AraResult<&ModelSpec> {
        self.registry
            .get(id)
            .ok_or_else(|| AraError::NotFound(format!("model '{id}' (see `ara models`)")))
    }
    fn profile(&self) -> AraResult<DeviceProfile> {
        ara_device_profiler::collect(self.store.root(), HostHints::default())
    }
    fn bench_path(&self) -> PathBuf {
        self.home.join("benchmarks.json")
    }
    fn benchmarks(&self) -> Vec<Benchmark> {
        std::fs::read_to_string(self.bench_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
    fn save_benchmark(&self, b: Benchmark) -> AraResult<()> {
        let mut all = self.benchmarks();
        all.retain(|x| !(x.model_id == b.model_id && x.context == b.context));
        all.push(b);
        std::fs::write(
            self.bench_path(),
            serde_json::to_string_pretty(&all).unwrap(),
        )?;
        Ok(())
    }
}

fn opt<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
}

fn num(args: &[String], name: &str, default: u32) -> AraResult<u32> {
    opt(args, name)
        .map(|v| {
            v.parse()
                .map_err(|_| AraError::InvalidInput(format!("{name} expects a number")))
        })
        .unwrap_or(Ok(default))
}

fn gib(b: u64) -> String {
    format!("{:.2} GiB", b as f64 / (1u64 << 30) as f64)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = run(&args) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(args: &[String]) -> AraResult<()> {
    let ctx = Ctx::new()?;
    match args.first().map(|s| s.as_str()) {
        Some("profile") => {
            let p = ctx.profile()?;
            println!("{}", serde_json::to_string_pretty(&p).unwrap());
            println!("ram class (hint only): {} GB", p.ram_class_gb());
        }
        Some("models") => models(&ctx, args)?,
        Some("download") => {
            let spec = ctx.spec(args.get(1).map(|s| s.as_str()).unwrap_or(""))?;
            let mut last = Instant::now();
            let started = Instant::now();
            let path = ctx.store.download(
                spec,
                &StoragePolicy::default(),
                &CancellationToken::new(),
                &mut |p| {
                    if last.elapsed().as_millis() > 1000 || p.downloaded == p.total {
                        last = Instant::now();
                        eprint!(
                            "\r{:5.1}%  {} / {}   ",
                            p.downloaded as f64 * 100.0 / p.total as f64,
                            gib(p.downloaded),
                            gib(p.total)
                        );
                    }
                },
            )?;
            eprintln!();
            println!(
                "installed and sha256-verified in {:.1}s: {}",
                started.elapsed().as_secs_f32(),
                path.display()
            );
        }
        Some("bench") => bench(&ctx, args)?,
        Some("chat") => chat(&ctx, args)?,
        Some("agent") => agent(&ctx, args)?,
        Some("verify") => verify(&ctx, args)?,
        Some("bench-ar") => bench_ar(&ctx, args)?,
        _ => {
            eprintln!("usage: ara <profile|models|download|bench|chat> ...");
            std::process::exit(2);
        }
    }
    Ok(())
}

fn models(ctx: &Ctx, args: &[String]) -> AraResult<()> {
    let mode = match opt(args, "--mode").unwrap_or("automatic") {
        "quality" => SelectionMode::Quality,
        "speed" => SelectionMode::Speed,
        _ => SelectionMode::Automatic,
    };
    let device = ctx.profile()?;
    let installed: BTreeSet<String> = ctx.store.installed_ids()?.into_iter().collect();
    let failed: std::collections::BTreeMap<String, BTreeSet<Capability>> =
        std::fs::read_to_string(ctx.home.join("verification.json"))
            .ok()
            .and_then(|s| {
                serde_json::from_str::<Vec<ara_coordinator::verify::VerifyReport>>(&s).ok()
            })
            .unwrap_or_default()
            .into_iter()
            .filter(|r| !r.tools_ok())
            .map(|r| (r.model_id, [Capability::ToolUse].into_iter().collect()))
            .collect();
    let req = SelectionRequest {
        failed_verification: failed,
        mode,
        capability: if args.iter().any(|a| a == "--tools") {
            Capability::ToolUse
        } else {
            Capability::Chat
        },
        language: Some(opt(args, "--lang").unwrap_or("ar").to_string()),
        context: num(args, "--ctx", 4096)?,
        installed: installed.clone(),
    };
    let r = ModelSelector::default().select(ctx.registry.iter(), &device, &req, &ctx.benchmarks());
    println!(
        "device: {} cores, RAM available {} of {}, free storage {}  | mode {:?}",
        device.cpu_cores,
        gib(device.available_ram_bytes),
        gib(device.total_ram_bytes),
        gib(device.free_storage_bytes),
        r.effective_mode.unwrap_or(mode)
    );
    println!("\ncan run (best first):");
    for (i, c) in r.candidates.iter().enumerate() {
        let s = ctx.spec(&c.model_id)?;
        println!(
            "  {}{:<30} file {:>9}  needs ~{:>9}  {}{}",
            if i == 0 { "★ " } else { "  " },
            c.model_id,
            gib(s.file_size_bytes),
            gib(c.required_memory_bytes),
            if c.estimated { "estimated" } else { "measured" },
            if installed.contains(&c.model_id) {
                "  [installed]"
            } else {
                ""
            }
        );
    }
    if !r.excluded.is_empty() {
        println!("\nnot suitable on this device:");
        for (id, why) in &r.excluded {
            println!("    {id:<30} {}", serde_json::to_string(why).unwrap());
        }
    }
    Ok(())
}

fn bench(ctx: &Ctx, args: &[String]) -> AraResult<()> {
    let spec = ctx.spec(args.get(1).map(|s| s.as_str()).unwrap_or(""))?;
    if ctx.store.state(spec) != InstallState::Installed {
        return Err(AraError::NotFound(
            "model not installed; run `ara download`".into(),
        ));
    }
    let context = num(args, "--ctx", 2048)?;
    let tokens = num(args, "--tokens", 64)?;
    let base = ara_device_profiler::process_memory()?;
    let t0 = Instant::now();
    let mut model = LlamaEngine.load(
        &ctx.store.model_path(&spec.id),
        &LoadParams {
            context,
            ..Default::default()
        },
    )?;
    let load_ms = t0.elapsed().as_millis() as u32;
    let msgs = [ChatMessage::new(
        Role::User,
        "Write a short paragraph about the history of the Nile river.",
    )];
    let params = GenerationParams {
        max_tokens: tokens,
        temperature: 0.0,
        ..Default::default()
    };
    let stats = model.generate_chat(&msgs, &params, &CancellationToken::new(), &mut |_| {})?;
    // Measure memory now: the comparison below loads extra model copies.
    let mem = ara_device_profiler::process_memory()?;
    // Follow-up turn: measures KV-cache reuse across a conversation.
    let mut answer = String::new();
    let long_ctx = [
        msgs[0].clone(),
        ChatMessage::new(
            Role::Assistant,
            "The Nile is the longest river in Africa.".repeat(20),
        ),
        ChatMessage::new(Role::User, "Summarize that in one sentence."),
    ];
    let cold = {
        // Fresh model = cold cache, same conversation.
        let mut fresh = LlamaEngine.load(
            &ctx.store.model_path(&spec.id),
            &LoadParams {
                context,
                ..Default::default()
            },
        )?;
        fresh.generate_chat(
            &long_ctx[..2]
                .iter()
                .cloned()
                .chain([ChatMessage::new(Role::User, "ok")])
                .collect::<Vec<_>>(),
            &GenerationParams {
                max_tokens: 1,
                temperature: 0.0,
                ..Default::default()
            },
            &CancellationToken::new(),
            &mut |_| {},
        )?;
        fresh.generate_chat(&long_ctx, &params, &CancellationToken::new(), &mut |t| {
            answer.push_str(t)
        })?
    };
    let mut cold_model = LlamaEngine.load(
        &ctx.store.model_path(&spec.id),
        &LoadParams {
            context,
            ..Default::default()
        },
    )?;
    let no_reuse =
        cold_model.generate_chat(&long_ctx, &params, &CancellationToken::new(), &mut |_| {})?;
    println!(
        "follow-up turn ({} prompt tokens): time to first token {} ms without cache vs {} ms with cache ({} tokens reused)",
        cold.prompt_tokens, no_reuse.time_to_first_token_ms, cold.time_to_first_token_ms, cold.reused_prompt_tokens
    );
    drop(cold_model);
    let b = Benchmark {
        model_id: spec.id.clone(),
        context,
        generation_tokens_per_sec: stats.tokens_per_sec,
        time_to_first_token_ms: stats.time_to_first_token_ms as u32,
        load_time_ms: load_ms,
        peak_memory_bytes: mem.peak_rss_bytes.saturating_sub(base.rss_bytes),
        stable: true,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "benchmark": b, "generation": stats,
        }))
        .unwrap()
    );
    ctx.save_benchmark(b)?;
    Ok(())
}

fn chat(ctx: &Ctx, args: &[String]) -> AraResult<()> {
    let spec = ctx.spec(args.get(1).map(|s| s.as_str()).unwrap_or(""))?;
    let message = args
        .get(2)
        .ok_or_else(|| AraError::InvalidInput("missing message".into()))?;
    let mut model = LlamaEngine.load(&ctx.store.model_path(&spec.id), &LoadParams::default())?;
    let mut msgs = Vec::new();
    if let Some(sys) = opt(args, "--system") {
        msgs.push(ChatMessage::new(Role::System, sys));
    }
    msgs.push(ChatMessage::new(Role::User, message.as_str()));
    let params = GenerationParams {
        max_tokens: num(args, "--max", 256)?,
        ..Default::default()
    };
    let cancel_after = opt(args, "--cancel-after").and_then(|v| v.parse::<usize>().ok());
    let cancel = CancellationToken::new();
    let mut chunks = 0usize;
    let stats = model.generate_chat(&msgs, &params, &cancel, &mut |t| {
        print!("{t}");
        let _ = std::io::stdout().flush();
        chunks += 1;
        if cancel_after.is_some_and(|n| chunks >= n) {
            cancel.cancel();
        }
    })?;
    println!();
    eprintln!("{}", serde_json::to_string(&stats).unwrap());
    Ok(())
}

fn agent(ctx: &Ctx, args: &[String]) -> AraResult<()> {
    use ara_coordinator::{Coordinator, IndexServices};
    use ara_tools::policy::{Grant, PermissionStore};
    use ara_tools::{skills, Permission, ToolRegistry};
    let spec = ctx.spec(args.get(1).map(|s| s.as_str()).unwrap_or(""))?;
    let message = args
        .get(2)
        .ok_or_else(|| AraError::InvalidInput("missing message".into()))?;
    let tools = ToolRegistry::with_builtins();
    let all = skills::builtin_skills();
    let persona = opt(args, "--persona")
        .map(ara_tools::personas::find)
        .transpose()?;
    let mut grants = PermissionStore::default();
    for p in [
        Permission::ComposeMessages,
        Permission::ShareContent,
        Permission::Alarms,
        Permission::LaunchApps,
        Permission::DeviceControls,
        Permission::OpenWeb,
        Permission::Calendar,
        Permission::Clipboard,
        Permission::PhoneDialer,
    ] {
        grants.set(p, Grant::AskEveryTime);
    }
    let mut index = ara_search::SearchIndex::open_in_memory()?;
    for (i, a) in args.iter().enumerate() {
        if a == "--doc" {
            let path = PathBuf::from(
                args.get(i + 1)
                    .ok_or_else(|| AraError::InvalidInput("--doc FILE".into()))?,
            );
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let d = index.add_document(&name, &std::fs::read(&path)?)?;
            eprintln!("indexed {} ({} chunks)", d.name, d.chunks);
        }
    }
    grants.set(Permission::ReadUserFiles, Grant::Always);
    let index = std::sync::Mutex::new(index);
    let services = IndexServices(&index);
    let mut model = LlamaEngine.load(&ctx.store.model_path(&spec.id), &LoadParams::default())?;
    let c = Coordinator {
        tools: &tools,
        skills: &all,
        grants: &grants,
        services: Some(&services),
        extra_system: "",
        persona: persona.as_ref(),
    };
    let params = GenerationParams {
        max_tokens: 200,
        temperature: 0.0,
        ..Default::default()
    };
    print!("ARA: ");
    let r = c.run_turn(
        &mut *model,
        &[],
        message,
        &params,
        &CancellationToken::new(),
        &mut |t| {
            print!("{t}");
            let _ = std::io::stdout().flush();
        },
    )?;
    println!();
    println!("routed skills: {:?}", r.routed_skills);
    for e in &r.tool_events {
        println!(
            "tool {} {} -> {}",
            e.call.tool,
            e.call.arguments,
            serde_json::to_string(&e.outcome).unwrap()
        );
    }
    if let Some(p) = &r.pending_confirmation {
        println!("waiting for confirmation: {}", p.summary);
    }
    for (i, s) in r.stats.iter().enumerate() {
        eprintln!(
            "[model turn {i}] prompt {} tokens, {:.1} tok/s",
            s.prompt_tokens, s.tokens_per_sec
        );
    }
    Ok(())
}

fn verify(ctx: &Ctx, args: &[String]) -> AraResult<()> {
    let spec = ctx.spec(args.get(1).map(|s| s.as_str()).unwrap_or(""))?;
    let mut model = LlamaEngine.load(
        &ctx.store.model_path(&spec.id),
        &LoadParams {
            context: 4096,
            ..Default::default()
        },
    )?;
    let r = ara_coordinator::verify::verify_model(&mut *model, &spec.id)?;
    println!("{}", serde_json::to_string_pretty(&r).unwrap());
    let path = ctx.home.join("verification.json");
    let mut all: Vec<ara_coordinator::verify::VerifyReport> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    all.retain(|x| x.model_id != r.model_id);
    all.push(r);
    std::fs::write(&path, serde_json::to_string_pretty(&all).unwrap())?;
    Ok(())
}

fn bench_ar(ctx: &Ctx, args: &[String]) -> AraResult<()> {
    let spec = ctx.spec(args.get(1).map(|s| s.as_str()).unwrap_or(""))?;
    let mut model = LlamaEngine.load(
        &ctx.store.model_path(&spec.id),
        &LoadParams {
            context: 2048,
            ..Default::default()
        },
    )?;
    let limit = opt(args, "--limit").and_then(|v| v.parse().ok());
    let r = ara_coordinator::arabic_bench::run(&mut *model, &spec.id, limit)?;
    println!(
        "{}: {}/{} = {:.0}%",
        r.model_id,
        r.passed,
        r.total,
        r.score() * 100.0
    );
    for (c, (p, t)) in &r.by_category {
        println!("  {c:<15} {p}/{t}");
    }
    if args.iter().any(|a| a == "--failures") {
        for f in &r.failures {
            println!("  ✗ {f}");
        }
    }
    let path = ctx.home.join("arabic_bench.json");
    let mut all: Vec<ara_coordinator::arabic_bench::ArabicReport> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    all.retain(|x| x.model_id != r.model_id);
    all.push(r);
    std::fs::write(&path, serde_json::to_string_pretty(&all).unwrap())?;
    Ok(())
}
