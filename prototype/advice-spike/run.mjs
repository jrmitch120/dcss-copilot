// PROTOTYPE. Runs every scenario × call type × setting × RUNS against the Anthropic API,
// sequentially (so latency isn't skewed by concurrency), and writes results.json.
//   node run.mjs --dry     write preview.md, no API calls
//   node run.mjs           real run (key from macOS keychain)
//   node run.mjs --only sigmund,haiku-off   filter by scenario id and/or setting id
import { execFileSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import { PREFIX } from "./prefix.mjs";
import { SCENARIOS } from "./scenarios.mjs";

const RUNS = 3;
const SETTINGS = [
  { id: "haiku-off", model: "claude-haiku-5-5", thinking: { type: "disabled" }, effort: "low" },
  { id: "haiku-low", model: "claude-haiku-5-5", effort: "low" },
  { id: "sonnet", model: "claude-sonnet-5-5" },
];
// USD per million tokens, from the LLM options research (2026-10-13).
const PRICE = {
  "claude-haiku-5-5": { in: 0.1, cw: 0.125, cr: 0.01, out: 0.5 },
  "claude-sonnet-5-5": { in: 2.0, cw: 2.5, cr: 0.1, out: 10.0 },
};

const diffOp = {
  type: "object",
  properties: {
    op: { type: "string", enum: ["add", "remove", "update"] },
    section: { type: "string", enum: ["route", "skill_targets", "needs", "goal"] },
    item: { type: "string" },
    detail: { type: "string" },
  },
  required: ["op", "section", "item", "detail"],
  additionalProperties: false,
};
const SCHEMAS = {
  tactical: {
    type: "object",
    properties: {
      choice: { type: "string", description: "A Candidate Action id" },
      agrees_with_suggested: { type: "boolean" },
      reason: { type: "string", description: "At most 25 words" },
    },
    required: ["choice", "agrees_with_suggested", "reason"],
    additionalProperties: false,
  },
  hints: {
    type: "object",
    properties: { nudge: { type: "string", description: "One sentence, at most 20 words" } },
    required: ["nudge"],
    additionalProperties: false,
  },
  coach: {
    type: "object",
    properties: {
      choice: { type: "string", description: "A Candidate Action id" },
      verdict: { type: "string" },
      reasoning: { type: "string", description: "At most 60 words" },
      run_plan_diff: { type: "array", items: diffOp },
    },
    required: ["choice", "verdict", "reasoning", "run_plan_diff"],
    additionalProperties: false,
  },
};
const callTypes = (s) => (s.kind === "tactical" ? ["tactical"] : ["hints", "coach"]);
const userMessage = (s, ct) => `Call type: ${ct}\n\n${s.sheet}`;

function body(setting, scenario, ct) {
  return {
    model: setting.model,
    max_tokens: 4000,
    stream: true,
    system: [{ type: "text", text: PREFIX, cache_control: { type: "ephemeral" } }],
    messages: [{ role: "user", content: userMessage(scenario, ct) }],
    ...(setting.thinking && { thinking: setting.thinking }),
    output_config: {
      ...(setting.effort && { effort: setting.effort }),
      format: { type: "json_schema", schema: SCHEMAS[ct] },
    },
  };
}

// ---- automatic checks ----
const candidateIds = (s) =>
  [...s.sheet.split("Candidate Actions:")[1].split("Suggested Action")[0].matchAll(/^\s{2}(\S+)\s{2,}/gm)].map((m) => m[1]);
const suggested = (s) => s.sheet.match(/Suggested Action \(rules\): (\S+)/)[1];
const numbersIn = (t) => (t.match(/\d+(?:[.,]\d+)*/g) || []).map((n) => n.replace(/,/g, ""));
function checks(scenario, ct, parsed) {
  if (!parsed) return { schema_ok: false };
  const prose = [parsed.reason, parsed.nudge, parsed.verdict, parsed.reasoning,
    ...(parsed.run_plan_diff || []).flatMap((d) => [d.item, d.detail])].filter(Boolean).join(" ");
  const allowed = new Set(numbersIn(userMessage(scenario, ct)));
  const words = prose.split(/\s+/).filter(Boolean).length;
  const limit = { tactical: 25, hints: 20, coach: 60 + 40 }[ct]; // coach: reasoning + verdict slack
  return {
    schema_ok: true,
    choice_valid: parsed.choice === undefined ? null : candidateIds(scenario).includes(parsed.choice),
    agrees_flag_honest: parsed.agrees_with_suggested === undefined ? null
      : parsed.agrees_with_suggested === (parsed.choice === suggested(scenario)),
    invented_numbers: numbersIn(prose).filter((n) => !allowed.has(n)),
    words, over_length: words > limit,
  };
}

// ---- one streamed call ----
async function call(key, setting, scenario, ct) {
  const t0 = performance.now();
  const res = await fetch("https://api.anthropic.com/v1/messages", {
    method: "POST",
    headers: { "x-api-key": key, "anthropic-version": "2023-06-01", "content-type": "application/json" },
    body: JSON.stringify(body(setting, scenario, ct)),
  });
  if (!res.ok) return { error: `${res.status} ${await res.text()}` };
  let ttft = null, ttfText = null, text = "", usage = {}, stop = null, thinkingChars = 0, buf = "";
  const dec = new TextDecoder();
  for await (const chunk of res.body) {
    buf += dec.decode(chunk, { stream: true });
    let i;
    while ((i = buf.indexOf("\n\n")) >= 0) {
      const ev = buf.slice(0, i); buf = buf.slice(i + 2);
      const data = ev.split("\n").find((l) => l.startsWith("data: "));
      if (!data) continue;
      const e = JSON.parse(data.slice(6));
      if (e.type === "message_start") Object.assign(usage, e.message.usage);
      if (e.type === "content_block_delta") {
        ttft ??= performance.now() - t0;
        if (e.delta.type === "text_delta") { ttfText ??= performance.now() - t0; text += e.delta.text; }
        if (e.delta.type === "thinking_delta") thinkingChars += e.delta.thinking.length;
      }
      if (e.type === "message_delta") { Object.assign(usage, e.usage); stop = e.delta.stop_reason; }
      if (e.type === "error") return { error: JSON.stringify(e.error) };
    }
  }
  const total = performance.now() - t0;
  let parsed = null;
  try { parsed = JSON.parse(text); } catch {}
  const p = PRICE[setting.model];
  const cost = ((usage.input_tokens || 0) * p.in + (usage.cache_creation_input_tokens || 0) * p.cw +
    (usage.cache_read_input_tokens || 0) * p.cr + (usage.output_tokens || 0) * p.out) / 1e6;
  return { ttft_ms: Math.round(ttft), ttf_text_ms: Math.round(ttfText), total_ms: Math.round(total),
    usage, cost_usd: cost, stop, thinking_chars: thinkingChars, text, parsed };
}

// ---- main ----
const args = process.argv.slice(2);
const only = args.includes("--only") ? args[args.indexOf("--only") + 1].split(",") : null;
const pick = (xs) => (only && xs.some((x) => only.includes(x.id)) ? xs.filter((x) => only.includes(x.id)) : xs);
const scenarios = pick(SCENARIOS), settings = pick(SETTINGS);

if (args.includes("--dry")) {
  let md = `# PROTOTYPE preview: every prompt the run will send\n\nStatic prefix: ${PREFIX.length} chars (~${Math.round(PREFIX.length / 2.5)} Claude tokens)\n\n`;
  for (const s of scenarios) for (const ct of callTypes(s)) {
    const u = userMessage(s, ct);
    md += `## ${s.title} — ${ct}\n\nCandidates: ${candidateIds(s).join(", ")} · Suggested: ${suggested(s)} · Expected: ${s.expected}\n\n${u.length} chars (~${Math.round(u.length / 2.5)} tokens)\n\n\`\`\`\n${u}\n\`\`\`\n\n`;
  }
  writeFileSync("preview.md", md);
  console.log(`wrote preview.md (${scenarios.length} scenarios, ${settings.length} settings, ${RUNS} runs)`);
  process.exit(0);
}

const key = execFileSync("security", ["find-generic-password", "-s", "dcss-copilot-spike", "-a", "anthropic", "-w"]).toString().trim();
const results = [];
// Grouped by setting so each model's cache stays warm, as it would in a real game.
for (const setting of settings) for (let run = 1; run <= RUNS; run++)
  for (const s of scenarios) for (const ct of callTypes(s)) {
    const r = await call(key, setting, s, ct);
    const row = { setting: setting.id, model: setting.model, scenario: s.id, call_type: ct, run, ...r,
      checks: r.error ? null : checks(s, ct, r.parsed) };
    results.push(row);
    console.log([setting.id, s.id, ct, `run${run}`, r.error ? `ERROR ${r.error.slice(0, 200)}` :
      `${r.total_ms}ms in=${r.usage.input_tokens} cw=${r.usage.cache_creation_input_tokens} cr=${r.usage.cache_read_input_tokens} out=${r.usage.output_tokens} $${r.cost_usd.toFixed(5)} ${r.parsed?.choice ?? ""} ${JSON.stringify(row.checks?.invented_numbers ?? [])}`].join("  "));
  }
writeFileSync("results.json", JSON.stringify({ when: new Date().toISOString(), settings: SETTINGS, results }, null, 2));
console.log(`wrote results.json (${results.length} calls)`);
