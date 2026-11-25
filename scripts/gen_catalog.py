import json, sys
hf = sys.argv[1]; out = sys.argv[2]
M = 1 << 20
# repo, file, id, name, version, quant, params_b, kv_bytes/token, max_ctx, langs, caps
entries = [
 ("Qwen/Qwen2.5-0.5B-Instruct-GGUF","qwen2.5-0.5b-instruct-q4_k_m.gguf","qwen2.5-0.5b-instruct-q4_k_m","Qwen2.5 0.5B Instruct","2.5","Q4_K_M",0.49,12288,32768,"qwen25",["chat"]),
 ("Qwen/Qwen2.5-1.5B-Instruct-GGUF","qwen2.5-1.5b-instruct-q4_k_m.gguf","qwen2.5-1.5b-instruct-q4_k_m","Qwen2.5 1.5B Instruct","2.5","Q4_K_M",1.54,28672,32768,"qwen25",["chat","tool_use","code"]),
 ("Qwen/Qwen3-0.6B-GGUF","Qwen3-0.6B-Q8_0.gguf","qwen3-0.6b-q8_0","Qwen3 0.6B","3","Q8_0",0.6,114688,32768,"qwen3",["chat","reasoning","tool_use"]),
 ("Qwen/Qwen3-1.7B-GGUF","Qwen3-1.7B-Q8_0.gguf","qwen3-1.7b-q8_0","Qwen3 1.7B","3","Q8_0",1.7,114688,32768,"qwen3",["chat","reasoning","tool_use","code"]),
 ("Qwen/Qwen3-4B-GGUF","Qwen3-4B-Q4_K_M.gguf","qwen3-4b-q4_k_m","Qwen3 4B","3","Q4_K_M",4.0,147456,32768,"qwen3",["chat","reasoning","tool_use","code"]),
 ("Qwen/Qwen3-8B-GGUF","Qwen3-8B-Q4_K_M.gguf","qwen3-8b-q4_k_m","Qwen3 8B","3","Q4_K_M",8.2,147456,32768,"qwen3",["chat","reasoning","tool_use","code"]),
 ("Qwen/Qwen2.5-Coder-1.5B-Instruct-GGUF","qwen2.5-coder-1.5b-instruct-q4_k_m.gguf","qwen2.5-coder-1.5b-instruct-q4_k_m","Qwen2.5 Coder 1.5B","2.5","Q4_K_M",1.54,28672,32768,"coder",["chat","code","tool_use"]),
 ("Qwen/Qwen2.5-Coder-7B-Instruct-GGUF","qwen2.5-coder-7b-instruct-q4_k_m.gguf","qwen2.5-coder-7b-instruct-q4_k_m","Qwen2.5 Coder 7B","2.5","Q4_K_M",7.6,57344,32768,"coder",["chat","code","tool_use"]),
 ("ibm-granite/granite-3.3-2b-instruct-GGUF","granite-3.3-2b-instruct-Q4_K_M.gguf","granite-3.3-2b-instruct-q4_k_m","IBM Granite 3.3 2B","3.3","Q4_K_M",2.5,81920,32768,"granite",["chat","tool_use","code","reasoning"]),
 ("ibm-granite/granite-4.0-micro-GGUF","granite-4.0-micro-Q4_K_M.gguf","granite-4.0-micro-q4_k_m","IBM Granite 4.0 Micro 3B","4.0","Q4_K_M",3.4,81920,32768,"granite",["chat","tool_use","code"]),
 ("ggml-org/SmolLM3-3B-GGUF","SmolLM3-Q4_K_M.gguf","smollm3-3b-q4_k_m","SmolLM3 3B","3","Q4_K_M",3.1,73728,32768,"smollm3",["chat","reasoning","tool_use"]),
 ("Qwen/Qwen3-14B-GGUF","Qwen3-14B-Q4_K_M.gguf","qwen3-14b-q4_k_m","Qwen3 14B (flagship)","3","Q4_K_M",14.8,163840,32768,"qwen3",["chat","reasoning","tool_use","code"]),
 ("HuggingFaceTB/SmolLM2-1.7B-Instruct-GGUF","smollm2-1.7b-instruct-q4_k_m.gguf","smollm2-1.7b-instruct-q4_k_m","SmolLM2 1.7B Instruct","2","Q4_K_M",1.7,196608,8192,"en",["chat"]),
]
# Languages as documented by the publishers (not yet evaluated by ARA).
LANGS = {
 "qwen25": ["ar","de","en","es","fr","it","ja","ko","pt","ru","th","vi","zh"],
 "qwen3":  ["ar","de","en","es","fr","hi","id","it","ja","ko","pt","ru","th","tr","ur","vi","zh"],
 "en": ["en"],
 "coder": ["en","zh"],
 "granite": ["ar","cs","de","en","es","fr","it","ja","ko","nl","pt","zh"],
 "smollm3": ["de","en","es","fr","it","pt"],
}
models = []
for repo, fn, mid, name, ver, q, pb, kv, mctx, lk, caps in entries:
    d = json.load(open(f"{hf}/{repo.replace('/','_')}.json"))
    sib = next(s for s in d["siblings"] if s["rfilename"] == fn)
    lic = (d.get("cardData") or {}).get("license")
    assert lic in ("apache-2.0", "mit"), (repo, lic)
    sha = sib["lfs"]["sha256"]; assert len(sha) == 64
    models.append({
        "id": mid, "display_name": name, "version": ver, "format": "gguf", "quantization": q,
        "parameters_b": pb, "file_size_bytes": sib["size"], "kv_bytes_per_token": kv,
        "runtime_overhead_bytes": 256 * M, "max_context": mctx, "default_context": 4096,
        "languages": sorted(LANGS[lk]), "capabilities": sorted(caps), "license": lic.replace("apache-2.0","Apache-2.0").replace("mit","MIT"),
        "source_url": f"https://huggingface.co/{repo}/resolve/main/{fn}", "sha256": sha,
    })
json.dump({"schema_version": 1, "models": models}, open(out, "w"), indent=2, ensure_ascii=False)
print(len(models), "models")
