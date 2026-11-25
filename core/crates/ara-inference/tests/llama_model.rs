//! Runs against a real GGUF model. Skipped unless ARA_TEST_MODEL points to
//! one (see docs/TESTING.md), so `cargo test` stays offline and fast.
#![cfg(feature = "llama")]

use ara_core_api::{AraError, CancellationToken};
use ara_inference::llama::LlamaEngine;
use ara_inference::{ChatMessage, GenerationParams, InferenceEngine, LoadParams, Role, StopReason};
use std::path::PathBuf;

fn model() -> Option<PathBuf> {
    std::env::var_os("ARA_TEST_MODEL").map(PathBuf::from)
}

fn greedy(max: u32) -> GenerationParams {
    GenerationParams {
        max_tokens: max,
        temperature: 0.0,
        ..Default::default()
    }
}

#[test]
fn real_model_streams_cancels_stops_and_is_deterministic() {
    let Some(path) = model() else {
        eprintln!("skipped: ARA_TEST_MODEL not set");
        return;
    };
    let mut m = LlamaEngine
        .load(
            &path,
            &LoadParams {
                context: 1024,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(m.context_len(), 1024);
    assert!(m.trained_context_len() >= 1024);
    assert!(m.count_tokens("مرحبا بالعالم").unwrap() > 0);

    let msgs = [ChatMessage::new(Role::User, "Say hello in Arabic.")];
    let mut a = String::new();
    let sa = m
        .generate_chat(&msgs, &greedy(24), &CancellationToken::new(), &mut |t| {
            a.push_str(t)
        })
        .unwrap();
    assert!(sa.generated_tokens > 0 && !a.is_empty());
    assert!(sa.time_to_first_token_ms >= sa.prompt_eval_ms);

    assert_eq!(sa.reused_prompt_tokens, 0, "fresh context");

    // Same prompt again: served from the KV cache and still identical
    // (proves cache reuse doesn't corrupt state).
    let mut b = String::new();
    let sb = m
        .generate_chat(&msgs, &greedy(24), &CancellationToken::new(), &mut |t| {
            b.push_str(t)
        })
        .unwrap();
    assert_eq!(a, b);
    assert_eq!(sb.reused_prompt_tokens, sb.prompt_tokens - 1);

    // Follow-up turn reuses the whole previous conversation prefix.
    let follow = [
        msgs[0].clone(),
        ChatMessage::new(Role::Assistant, b.clone()),
        ChatMessage::new(Role::User, "And in French?"),
    ];
    let sf = m
        .generate_chat(&follow, &greedy(8), &CancellationToken::new(), &mut |_| {})
        .unwrap();
    assert!(sf.reused_prompt_tokens * 2 > sf.prompt_tokens, "{sf:?}");

    // Cancellation from inside the stream.
    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    let s = m
        .generate_chat(
            &[ChatMessage::new(Role::User, "Count from 1 to 200.")],
            &greedy(300),
            &cancel,
            &mut |_| c2.cancel(),
        )
        .unwrap();
    assert_eq!(s.stop_reason, StopReason::Cancelled);
    assert!(s.generated_tokens <= 2);

    // Stop sequence is honoured and never emitted.
    let mut out = String::new();
    let p = GenerationParams {
        stop: vec!["5".into()],
        ..greedy(200)
    };
    let s = m
        .generate_chat(
            &[ChatMessage::new(
                Role::User,
                "Count from 1 to 10 separated by spaces.",
            )],
            &p,
            &CancellationToken::new(),
            &mut |t| out.push_str(t),
        )
        .unwrap();
    assert_eq!(s.stop_reason, StopReason::StopSequence, "{out}");
    assert!(!out.contains('5'));

    // Prompt larger than the context is rejected, not truncated silently.
    let long = "word ".repeat(3000);
    let err = m
        .generate_chat(
            &[ChatMessage::new(Role::User, long)],
            &greedy(4),
            &CancellationToken::new(),
            &mut |_| {},
        )
        .unwrap_err();
    assert!(matches!(err, AraError::InsufficientResources(_)));
}

#[test]
fn missing_and_corrupt_files_fail_cleanly() {
    let dir = std::env::temp_dir().join("ara-corrupt-test");
    std::fs::create_dir_all(&dir).unwrap();
    let bad = dir.join("bad.gguf");
    std::fs::write(&bad, b"not a gguf file").unwrap();
    let e = LlamaEngine
        .load(&bad, &LoadParams::default())
        .err()
        .unwrap();
    assert!(matches!(e, AraError::InvalidInput(_)));
    assert!(!e.to_string().contains("ara-corrupt-test"), "no path leak");
    let e = LlamaEngine
        .load(&dir.join("missing.gguf"), &LoadParams::default())
        .err()
        .unwrap();
    assert!(matches!(e, AraError::NotFound(_)));
}
