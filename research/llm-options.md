# LLM options: cost, cheap and local models, swappable providers

Research for issue #6 (map: #1). Prices and docs checked on **2026-10-13**. Prices move often, so
re-check the linked pages before you rely on a number.

## Question

For a serialized Game Situation, what does an LLM call realistically cost across candidate
models (Anthropic Haiku/Sonnet-class, other cloud providers, local models via Ollama/llama.cpp)?
What latency should we expect? Which Rust and TypeScript libraries give a swappable provider
abstraction (streaming, tool use, structured output)? Can the default settings cost **under ~$1
per long game**, and at what call frequency?

## Answer in brief

- **A call costs about $0.0004 on the cheapest current cloud tier** (Claude Haiku 5.5, OpenAI
  gpt-6-luna, Gemini 2.5 Flash-Lite) when the static prefix is cached. Sonnet-class models cost
  about **$0.007** a call, roughly 20x more.
- **Under $1 per long game works with a cheap model and event-driven calls** (about 2,000 calls
  in a 50k-turn game, about $0.70). It also works with a Sonnet-class model if the Advisor is
  on-demand only (about 140 calls fit in $1). **A call every turn does not fit the target on any
  cloud model:** even the cheapest costs about $17 to $18 per game.
- **Input tokens drive the cost, not output.** Once the static prefix is cached, the changing
  Game Situation (about 2k tokens) is most of every bill. The biggest savings come from keeping
  the per-call payload small (send diffs, cache Accumulated State) and from calling less often.
- **Local models cost nothing per call but are slow on typical hardware.** A 7B Q4 model on a
  2023-era M2 Pro processes prompts at about 294 tokens/s and generates at about 38 tokens/s, so a
  call with 2k new tokens and a 200-token answer takes about 12 s. On an M4 Max it takes about 5 s.
  Local models are fine as an option but too slow as the default for tactical advice.
- **Library options:** Rust has **rig** (`rig-core` 0.44, 28 provider modules including
  anthropic, openai, gemini, ollama, llamacpp, openrouter, deepseek) and **genai** (0.6, about 35
  adapters, with Anthropic `cache_control` built in). TypeScript has the **Vercel AI SDK**
  (`ai` with `@ai-sdk/*` providers, the de facto standard, with Anthropic cache control and
  structured output), plus **TanStack AI** and **LangChain.js**. Both Ollama and llama-server
  expose OpenAI-compatible endpoints, so one "OpenAI-compatible" adapter covers every local
  runtime.

## 1. Token estimate for a serialized Game Situation

I wrote a realistic compact serialization of a mid/late-game Game Situation: a level 18
Minotaur Fighter on Depths:3 at turn 18,432. It includes the character sheet, equipment, a
22-item inventory, a 7-row view of the map, 4 monster groups, 12 recent messages, an Accumulated
State summary (runes, branches, uniques, stashes, shops) and Game Knowledge snippets for the
monsters in view.

| Measure | Value |
|---|---|
| Characters | 2,528 |
| Words | 403 |
| Lines | 45 |

Converting characters to tokens depends on the tokenizer:

- Anthropic says its current tokenizer (introduced with Claude Opus 4.7) fits "1M tokens is
  roughly 555k words or 2.5M Unicode characters"
  ([models overview](https://platform.claude.com/docs/en/about-claude/models/overview)). That is
  about **2.5 chars/token**, so the sample is about **1,000 tokens** on Claude.
- At a typical ~4 chars/token for other tokenizers, it is about 630 tokens. ASCII map glyphs and
  terse stat strings usually tokenize worse than prose, so expect 700 to 1,000.

That sample is a minimal situation. A real one would carry the full visible map (LOS radius 7
gives a 15x15 grid), more messages, the whole inventory, and a fuller Accumulated State. So the
model below uses:

| Component | Tokens | Cacheable? |
|---|---|---|
| System prompt, Advisor rules, tool/output schemas, general strategy notes | ~4,000 | Yes. The same prefix on every call. |
| Per-call Game Situation (Visible State, Accumulated State, Game Knowledge for what is in view) | ~2,000 | No, it changes every call (see section 5 for partial caching) |
| Output (short Tactical Advice) | ~200 | n/a |

If a model thinks before it answers, thinking tokens are billed as output. Current Claude models
use adaptive thinking with a default effort of `medium` (Haiku 5.5) or `high` (Sonnet 5.5)
([models overview](https://platform.claude.com/docs/en/about-claude/models/overview)). For
frequent tactical calls, set effort low or turn thinking off. Otherwise the 200-token output
assumption breaks.

## 2. Prices (USD per million tokens, as of 2026-10-13)

| Provider / model | Input | Cache write | Cache read | Output | Source |
|---|---|---|---|---|---|
| Claude Haiku 5.5 (prompts up to 100k) | 0.10 | 0.125 (5m) / 0.20 (1h) | 0.01 | 0.50 | [Anthropic pricing](https://platform.claude.com/docs/en/about-claude/pricing) |
| Claude Haiku 4.5 | 1.00 | 1.25 / 2.00 | 0.10 | 5.00 | same |
| Claude Sonnet 5.5 | 2.00 | 2.50 / 4.00 | 0.10 | 10.00 | same |
| Claude Opus 5.5 | 4.00 | 5.00 / 8.00 | 0.20 | 20.00 | same |
| OpenAI gpt-6-luna | 0.10 | 1.25x input (assumed, see note) | 0.01 | 0.50 | [OpenAI pricing](https://developers.openai.com/api/docs/pricing) |
| OpenAI gpt-5.4-nano | 0.20 | none | 0.02 | 1.25 | same |
| OpenAI gpt-5.4-mini | 0.75 | none | 0.075 | 4.50 | same |
| Gemini 2.5 Flash-Lite | 0.10 | implicit | 0.01 | 0.40 | [Gemini pricing](https://ai.google.dev/gemini-api/docs/pricing) |
| Gemini 3.1 Flash-Lite | 0.25 | implicit | 0.025 | 1.50 | same |
| Gemini 3.8 Flash (through 2026-12-31) | 0.75 | implicit | 0.075 | 3.75 | same |
| Gemini 3.8 Flash (from 2027-01-01) | 1.50 | implicit | 0.15 | 7.50 | same |
| DeepSeek `deepseek-flash` (V4.1-Flash), peak | 0.30 | n/a | 0.006 | 1.20 | [DeepSeek pricing](https://api-docs.deepseek.com/quick_start/pricing) |
| Local (Ollama / llama.cpp) | 0 | 0 | 0 | 0 | electricity and hardware only |

Notes:

- Batch APIs (Anthropic, OpenAI) give 50% off, but they are asynchronous, so they don't suit
  real-time advice ([Anthropic pricing, Batch processing](https://platform.claude.com/docs/en/about-claude/pricing)).
- Claude Haiku 5.5 is priced by prompt length. A prompt over 100k tokens pays 5x ($0.50 in /
  $2.50 out), so keep the Game Situation well under 100k ([Anthropic pricing](https://platform.claude.com/docs/en/about-claude/pricing)).
- DeepSeek's off-peak rates are half the peak rates. Peak hours are 01:00-04:00 and 06:00-10:00
  UTC on weekdays ([DeepSeek pricing](https://api-docs.deepseek.com/quick_start/pricing)).
- Several Gemini prices double on 2026-12-01 (Gemini 4 Argon) or 2027-01-01 (3.8 and 3.6 Flash)
  ([Gemini pricing](https://ai.google.dev/gemini-api/docs/pricing)).
- OpenAI: from GPT-5.6 on, "cache writes cost 1.25x the standard, uncached input-token rate".
  Earlier models have "No additional cache-write charge"
  ([OpenAI prompt caching](https://developers.openai.com/api/docs/guides/prompt-caching)). I
  assumed gpt-6-luna follows the GPT-5.6+ rule.

### Prompt caching rules that matter here

| Provider | How | Minimum cacheable | Lifetime | Source |
|---|---|---|---|---|
| Anthropic | Explicit `cache_control` breakpoints (up to 4) or automatic | **512 tokens** on Haiku 5.5, Sonnet 5.5, Opus 5.5; 4,096 on Haiku 4.5; 1,024 on Sonnet 5 / 4.6 | 5 min, refreshed free on each hit; optional 1 h at 2x input for writes | [Prompt caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching) |
| OpenAI | Automatic | 1,024 tokens (GPT-5.6+) | 30 min after the last write or reuse (GPT-5.6+) | [Prompt caching](https://developers.openai.com/api/docs/guides/prompt-caching) |
| Gemini | Implicit, automatic on 2.5+ | 4,096 (3.x Flash/Pro, 4 Argon); 2,048 (2.5 Flash/Pro) | not stated | [Context caching](https://ai.google.dev/gemini-api/docs/caching) |
| llama.cpp | `cache_prompt` (on by default) reuses the KV cache for a shared prefix | none | while the slot lives | [llama-server README](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md) |
| Ollama | Reports `prompt_eval_cached_count` | none | the model stays loaded 5 min by default (`keep_alive`) | [Ollama chat API](https://docs.ollama.com/api/chat), [FAQ](https://docs.ollama.com/faq) |

Two consequences. First, Gemini 3.x needs a **4,096-token** shared prefix before implicit caching
starts, so a ~4k static prefix sits right at the threshold. Second, Anthropic's 5-minute TTL is
refreshed by every hit, so the cache stays warm through normal play. Only longer pauses cause a
re-write.

## 3. Cost per long game (50,000 turns)

Model assumptions: 4,000-token cached prefix, 2,000 uncached tokens per call, 200 output
tokens, and 5% of calls re-writing the prefix cache (after pauses or prompt edits). The
calculation script is reproduced in the appendix.

Call frequencies:

- **Every turn**: 50,000 calls.
- **Every 10 turns**: 5,000 calls.
- **Event-driven**: about 2,000 calls. Trigger on things like a new monster in view, HP below a
  threshold, a level change, or a notable item. This count is an assumption; Game Source
  research should measure real event rates.
- **On-demand only**: about 200 player questions per game.

| Model | $/call (cached) | $/call (no cache) | Every turn (50k) | Every 10 turns (5k) | Event-driven (~2k) | On-demand (~200) |
|---|---|---|---|---|---|---|
| Claude Haiku 5.5 | 0.00036 | 0.00070 | $18.15 | $1.81 | **$0.73** | **$0.07** |
| Claude Haiku 4.5 | 0.00363 | 0.00700 | $181.50 | $18.15 | $7.26 | **$0.73** |
| Claude Sonnet 5.5 | 0.00688 | 0.01400 | $344.00 | $34.40 | $13.76 | $1.38 |
| Claude Opus 5.5 | 0.01376 | 0.02800 | $688.00 | $68.80 | $27.52 | $2.75 |
| OpenAI gpt-6-luna | 0.00036 | 0.00070 | $18.15 | $1.81 | **$0.73** | **$0.07** |
| OpenAI gpt-5.4-nano | 0.00077 | 0.00145 | $38.30 | $3.83 | $1.53 | **$0.15** |
| OpenAI gpt-5.4-mini | 0.00283 | 0.00540 | $141.75 | $14.17 | $5.67 | **$0.57** |
| Gemini 2.5 Flash-Lite | 0.00034 | 0.00068 | $16.90 | $1.69 | **$0.68** | **$0.07** |
| Gemini 3.1 Flash-Lite | 0.00094 | 0.00180 | $47.25 | $4.72 | $1.89 | **$0.19** |
| Gemini 3.8 Flash (2026 price) | 0.00268 | 0.00525 | $134.25 | $13.42 | $5.37 | **$0.54** |
| Gemini 3.8 Flash (2027 price) | 0.00537 | 0.01050 | $268.50 | $26.85 | $10.74 | $1.07 |
| DeepSeek flash (peak) | 0.00092 | 0.00204 | $46.14 | $4.61 | $1.85 | **$0.18** |
| Local 7B-class | $0 | $0 | $0 | $0 | $0 | $0 |

Bold means under $1 per long game.

What the table shows:

- **Caching roughly halves the cost.** It can't do more because the 2k uncached tokens per call
  remain. On Haiku 5.5, about 56% of a cached call's cost is the uncached Game Situation and 28%
  is output.
- **Under $1 per game works only with a cheap tier and event-driven or on-demand calls.** At
  every 10 turns, even the cheapest models land at about $1.70 to $1.80.
- **A tiered default fits the budget.** Use a cheap model for event-driven Tactical Advice
  (2,000 x $0.00036 = $0.73) and a Sonnet-class model for 30 to 40 Strategic Advice or on-demand
  questions (about $0.21 to $0.28). The total is about $0.95 to $1.00.

## 4. Latency

**Cloud.** No provider publishes absolute latency or throughput for these models. Anthropic labels
comparative latency only: Haiku 5.5 is "Fastest", Sonnet 5.5 "Fast", Opus 5.5 "Moderate",
Fable 5.1 "Slower". It also notes that "actual latency depends on prompt length, output length,
and thinking effort" ([models overview](https://platform.claude.com/docs/en/about-claude/models/overview)).
I found no first-party numbers to cite, so **a spike should measure time-to-first-token and total
time** for the shortlisted models with a real ~6k-token prompt. Prompt caching also speeds up
prefill, and streaming shows the first words of advice before the full answer arrives.

**Local.** The llama.cpp Apple Silicon benchmark ([discussion #4167](https://github.com/ggml-org/llama.cpp/discussions/4167))
measures Llama 2 7B at Q4_0. Prompt processing is `pp512`; generation is `tg128`:

| Chip | Prompt t/s | Gen t/s | 2k new prompt tokens + 200 out | 6k cold prompt + 200 out |
|---|---|---|---|---|
| M1 (7-core GPU) | 108 | 14 | ~33 s | ~70 s |
| M2 Pro (16) | 294 | 38 | ~12 s | ~26 s |
| M4 Pro (20) | 440 | 51 | ~8.5 s | ~18 s |
| M4 Max (40) | 886 | 83 | ~4.7 s | ~9 s |

The last two columns are derived: tokens divided by the measured rates. The "2k new" column
assumes the 4k prefix is served from the KV cache (`cache_prompt` in llama.cpp, shown in Ollama
as `prompt_eval_cached_count`). Larger models (14B to 32B), which give advice closer to cloud
quality, are proportionally slower. Discrete NVIDIA GPUs are faster. In short, local works for
on-demand questions and slow-paced play, but is marginal for real-time Tactical Advice on common
laptops.

**Ollama gotchas.** "By default, Ollama uses a context window size of 4096 tokens", and models are
"kept in memory for 5 minutes before being unloaded" ([Ollama FAQ](https://docs.ollama.com/faq)).
A ~6k-token Game Situation would be **silently truncated** unless we set `num_ctx` (8192 or more)
per request. We should also set `keep_alive` so the model isn't reloaded after a pause.

## 5. Ways to cut cost further (design levers)

1. **Call on events, not every turn.** This is the largest lever: 25x fewer calls than every
   turn.
2. **Stack cache breakpoints by rate of change.** Order the prompt as static prefix, then
   Accumulated State (changes every few hundred turns), then Visible State (changes every turn).
   Anthropic allows up to 4 breakpoints, which moves about 1k more tokens to the cache-read rate.
3. **Send only what a call needs.** Trim the map to nearby cells, include Game Knowledge only for
   monsters and items in view, and keep only the last N messages.
4. **Cap output and thinking.** Use short structured advice, `max_tokens` around 300, and
   low effort or no thinking for tactical calls.
5. **Escalate by tier.** A cheap model triages; a stronger model answers only when the cheap
   one flags danger or the player asks.

## 6. Swappable-provider libraries

Requirements: streaming, tool use, structured output, provider prompt caching, and local runtimes.

### Rust

| Library | Version / activity | Providers | Streaming | Tools | Structured output | Prompt caching | Notes |
|---|---|---|---|---|---|---|---|
| **rig** ([0xPlaygrounds/rig](https://github.com/0xPlaygrounds/rig), MIT, ~8.9k stars, pushed 2026-10-13) | `rig-core` 0.44.0 ([docs.rs](https://docs.rs/rig-core/latest/rig_core/)) | 28 modules: anthropic, azure, chatgpt, cohere, copilot, deepseek, gemini, groq, huggingface, **llamacpp**, mistral, moonshot, **ollama**, openai, **openrouter**, perplexity, together, xai, zai, ... ([providers](https://docs.rs/rig-core/latest/rig_core/providers/index.html)) | Yes (`streaming` module) | Yes (`tool` module) | Yes (`structured_output` module) | Not documented on the pages I read | Agent-framework style, so it carries more abstraction. Pre-1.0, so expect API churn. |
| **genai** ([jeremychone/rust-genai](https://github.com/jeremychone/rust-genai), Apache-2.0, ~900 stars, pushed 2026-10-10) | 0.6 current, 0.7 in beta | about 35 adapters including openai, anthropic, gemini, **ollama**, deepseek, groq, open_router, bedrock, vertex, xai | Yes | Yes | Yes (JSON schema normalized for Gemini) | **Yes**: `ChatOptions::with_cache_control` for Anthropic, `prompt_cache_key` for OpenAI | A thin client, closest to "just swap the provider". Custom endpoints via `ServiceTargetResolver`, so llama-server works through the OpenAI adapter. Single maintainer. |
| **async-openai** ([64bit/async-openai](https://github.com/64bit/async-openai), MIT, ~2k stars) | active | OpenAI-shaped APIs only | Yes | Yes | Yes | n/a | Use only if every target speaks the OpenAI API. That covers Ollama, llama-server, DeepSeek and OpenRouter, but not native Anthropic caching. |
| graniet/llm ([repo](https://github.com/graniet/llm), MIT, ~365 stars) | last push 2026-06-06 | multi | | | | | Less active, so lower priority. |

### TypeScript

| Library | Activity | Providers | Streaming / tools / structured output | Prompt caching | Notes |
|---|---|---|---|---|---|
| **Vercel AI SDK** ([vercel/ai](https://github.com/vercel/ai), ~27k stars, pushed 2026-10-13) | very active | Official: OpenAI, Anthropic, Google, Vertex, Mistral, DeepSeek, Groq, xAI, Together, Fireworks, Cerebras, ... ([providers](https://ai-sdk.dev/providers/ai-sdk-providers)). `@ai-sdk/openai-compatible` for any OpenAI-compatible endpoint ([docs](https://ai-sdk.dev/providers/openai-compatible-providers)). Community Ollama providers: `ai-sdk-ollama`, `ollama-ai-provider-v2` ([docs](https://ai-sdk.dev/providers/community-providers/ollama)) | Yes: `streamText`, `tool()`, `Output` for structured output. Anthropic `structuredOutputMode` uses native structured outputs on Sonnet/Opus 5.5 | **Yes**: `providerOptions.anthropic.cacheControl` (including `ttl: '1h'`); cache read/write counts in `usage.inputTokenDetails` ([Anthropic provider](https://ai-sdk.dev/providers/ai-sdk-providers/anthropic)) | The de facto standard. The license field reads NOASSERTION on GitHub; check the LICENSE file. |
| **TanStack AI** ([TanStack/ai](https://github.com/TanStack/ai), MIT, ~3.2k stars) | very active | OpenAI, Anthropic, Gemini, ... | Streaming, tool calling ("type-safe, provider-agnostic") | Not checked | Newer and smaller ecosystem |
| **LangChain.js** ([langchain-ai/langchainjs](https://github.com/langchain-ai/langchainjs), MIT, ~18k stars) | very active | broad | Yes | Yes | Heavy, and more than this project needs |

### Local runtimes, for any of the above

- **Ollama**: native `/api/chat` with streaming, `tools`, and `format` (`"json"` or a JSON Schema
  object) ([chat API](https://docs.ollama.com/api/chat)). It also serves an OpenAI-compatible
  `/v1/chat/completions` (streaming, JSON mode, tools, `response_format`) and `/v1/responses`
  ([OpenAI compatibility](https://docs.ollama.com/api/openai-compatibility)).
- **llama.cpp `llama-server`**: OpenAI-compatible `/v1/chat/completions` with streaming, tool
  calling via `--jinja` (on by default), `response_format` with `json_schema`, GBNF grammars,
  and `cache_prompt` KV prefix reuse ([README](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)).
- So **one OpenAI-compatible adapter plus a native Anthropic adapter** (for caching) covers
  everything. OpenRouter, which rig and genai both support, can stand in for many cloud
  providers with a single key.

## 7. Recommended candidates

| Role | First choice | Alternatives |
|---|---|---|
| Default, event-driven Tactical Advice | Claude Haiku 5.5 ($0.10 / $0.50, 512-token cache minimum) | gpt-6-luna (same price), Gemini 2.5 Flash-Lite (cheapest, older), DeepSeek flash off-peak |
| Strategic Advice and on-demand questions | Claude Sonnet 5.5 ($2 / $10, cache read $0.10) | gpt-5.4-mini, Gemini 3.8 Flash (watch the Jan 2027 doubling) |
| Offline / free | Ollama or llama-server with a 7B-14B instruct model, `num_ctx` of 8192 or more | |

A default of "cheap model on events, Sonnet-class on demand" lands at about **$0.75 to $1.00 per
50k-turn game**.

## Open questions and caveats

- **Advice quality** of the cheap tiers on DCSS tactics is untested. Cost is moot if Haiku-class
  or Flash-Lite-class advice is poor, so an eval spike should compare a cheap tier and Sonnet on
  saved Game Situations.
- **Real event rate** per game is unknown. The 2,000-call figure is an assumption to check with
  Game Source data.
- **Cloud latency** has no first-party figures. It needs measurement.
- Tokens per Game Situation come from a character-count estimate (I had no tokenizer or API key).
  Anthropic's `count_tokens` endpoint on real serializations would firm this up.
- The local throughput figures are for Llama 2 7B (a 2023 model) on llama.cpp. Current models of
  the same size and quantization should be in the same range, but this is not measured.

## Appendix: cost script

```python
S, D, O = 4000, 2000, 200   # cached prefix, per-call dynamic input, output tokens
WRITE_FRACTION = 0.05       # share of calls that re-write the prefix cache
# name: (input, cache_write, cache_read, output) USD/MTok
def per_call(p, cached=True):
    i, w, r, o = p
    if not cached:
        return ((S + D) * i + O * o) / 1e6
    prefix = S * (WRITE_FRACTION * w + (1 - WRITE_FRACTION) * r)
    return (prefix + D * i + O * o) / 1e6
# game cost = per_call(p) * calls, with calls in {50000, 5000, 2000, 200}
```
