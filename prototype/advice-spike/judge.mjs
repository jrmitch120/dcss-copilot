// PROTOTYPE. Builds judge.html from results.json: one randomly chosen run per
// scenario × call type × setting, shuffled into blind A/B/C slots.
import { readFileSync, writeFileSync } from "node:fs";
import { SCENARIOS } from "./scenarios.mjs";

const { results } = JSON.parse(readFileSync("results.json", "utf8"));
const shuffle = (xs) => xs.map((x) => [Math.random(), x]).sort((a, b) => a[0] - b[0]).map((p) => p[1]);
const items = [];
for (const s of SCENARIOS) for (const ct of [...new Set(results.filter((r) => r.scenario === s.id).map((r) => r.call_type))]) {
  const bySetting = Object.groupBy(results.filter((r) => r.scenario === s.id && r.call_type === ct && r.parsed), (r) => r.setting);
  const outputs = shuffle(Object.values(bySetting).map((runs) => runs[Math.floor(Math.random() * runs.length)]));
  items.push({ key: `${s.id}/${ct}`, title: s.title, ct, sheet: s.sheet, expected: s.expected, trap: s.trap,
    suggested: s.sheet.match(/Suggested Action \(rules\): (\S+)/)[1],
    outputs: outputs.map((r) => ({ setting: r.setting, run: r.run, parsed: r.parsed, checks: r.checks })) });
}

const html = `<!doctype html><meta charset="utf-8"><title>PROTOTYPE — advice quality judging</title>
<style>
body{font:14px/1.45 system-ui;background:#111;color:#ddd;max-width:1200px;margin:auto;padding:20px}
h1{font-size:18px} .banner{background:#5319e7;color:#fff;padding:6px 10px;border-radius:6px}
.item{border:1px solid #333;border-radius:8px;margin:18px 0;padding:12px} pre{white-space:pre-wrap;background:#1a1a1a;padding:10px;font-size:12px}
.outs{display:grid;grid-template-columns:repeat(3,1fr);gap:10px} .out{background:#1b1b22;border-radius:6px;padding:10px}
.flag{color:#ff6b6b;font-size:12px} .ok{color:#7bd389;font-size:12px} label{display:block;margin:3px 0}
.hidden{display:none} table{border-collapse:collapse;margin-top:10px} td,th{border:1px solid #444;padding:4px 8px}
button{background:#333;color:#eee;border:1px solid #555;padding:6px 12px;border-radius:6px;cursor:pointer}
.reveal-only{color:#f5c542}
</style>
<h1>DCSS Copilot — advice quality spike <span class="banner">PROTOTYPE · blind judging</span></h1>
<p>Rate every output. Settings are hidden behind A/B/C (shuffled per item). Ratings autosave in this browser. Expected picks show after Reveal.
<span id="progress"></span></p>
<div id="items"></div>
<p><button onclick="reveal()">Reveal settings + summary</button> <button onclick="copyRatings()">Copy ratings</button></p>
<div id="summary"></div>
<script>
const ITEMS = ${JSON.stringify(items)};
const RESULTS = ${JSON.stringify(results.map(({ text, ...r }) => r))};
const store = JSON.parse(localStorage.getItem("advice-spike-ratings") || "{}");
const save = () => { localStorage.setItem("advice-spike-ratings", JSON.stringify(store)); progress(); };
const esc = (s) => String(s).replace(/[&<>]/g, (c) => ({"&":"&amp;","<":"&lt;",">":"&gt;"}[c]));
function radio(id, field, opts) {
  return opts.map(o => '<label><input type=radio name="'+id+field+'" '+(store[id]?.[field]===o?'checked':'')+
    ' onchange="(store[\\''+id+'\\']??={}).'+field+'=\\''+o+'\\';save()"> '+o+'</label>').join("");
}
function renderOut(it, o, i) {
  const id = it.key + "#" + o.setting, p = o.parsed, c = o.checks;
  let body = "";
  if (p.choice !== undefined) body += "<b>choice:</b> "+esc(p.choice)+(p.choice===it.suggested?" <i>(= suggested)</i>":"")+"<br>";
  if (p.reason) body += "<b>reason:</b> "+esc(p.reason)+"<br>";
  if (p.nudge) body += "<b>nudge:</b> "+esc(p.nudge)+"<br>";
  if (p.verdict) body += "<b>verdict:</b> "+esc(p.verdict)+"<br><b>reasoning:</b> "+esc(p.reasoning)+"<br>";
  if (p.run_plan_diff) body += "<b>run plan diff:</b><ul>"+p.run_plan_diff.map(d=>"<li>"+esc(d.op+" "+d.section+": "+d.item+" — "+d.detail)+"</li>").join("")+"</ul>";
  const flags = [c.choice_valid===false&&"choice NOT a Candidate Action", c.invented_numbers?.length&&"numbers not on sheet: "+c.invented_numbers.join(", "),
    c.over_length&&"over length ("+c.words+" words)", c.agrees_flag_honest===false&&"agrees_with_suggested flag is wrong"].filter(Boolean);
  const rate = it.ct === "hints"
    ? "<b>nudge quality</b>"+radio(id,"quality",["1 poor","2 ok","3 good"])
    : "<b>choice</b>"+radio(id,"choice",["acceptable","questionable","dangerous"])+"<b>reason quality</b>"+radio(id,"quality",["1 poor","2 ok","3 good"])+
      "<b>better than Suggested Action alone?</b>"+radio(id,"better",["yes","no"]);
  return '<div class=out><b>'+"ABC"[i]+'</b> <span class="reveal-only hidden">'+o.setting+' (run '+o.run+')</span><p>'+body+'</p>'+
    (flags.length?'<div class=flag>⚠ '+flags.join("<br>⚠ ")+'</div>':'<div class=ok>auto-checks clean</div>')+'<hr>'+rate+'</div>';
}
document.getElementById("items").innerHTML = ITEMS.map(it =>
  '<div class=item><h3>'+esc(it.title)+' — <code>'+it.ct+'</code></h3><details><summary>Fact Sheet</summary><pre>'+esc(it.sheet)+'</pre></details>'+
  '<p>Suggested Action: <code>'+it.suggested+'</code> <span class="reveal-only hidden">· expected: <code>'+it.expected+'</code>'+(it.trap?' · TRAP':'')+'</span></p>'+
  '<div class=outs>'+it.outputs.map((o,i)=>renderOut(it,o,i)).join("")+'</div></div>').join("");
function progress() {
  const need = ITEMS.reduce((n,it)=>n+it.outputs.length*(it.ct==="hints"?1:3),0);
  const have = Object.values(store).reduce((n,r)=>n+Object.keys(r).length,0);
  document.getElementById("progress").textContent = " Rated "+have+" / "+need+".";
}
progress();
const med = (xs) => { const s=[...xs].sort((a,b)=>a-b); return s.length? s[Math.floor((s.length-1)/2)] : NaN; };
function reveal() {
  document.querySelectorAll(".reveal-only").forEach(e=>e.classList.remove("hidden"));
  const settings = [...new Set(RESULTS.map(r=>r.setting))];
  const rows = settings.map(st => {
    const rs = RESULTS.filter(r=>r.setting===st && !r.error), tac = rs.filter(r=>r.call_type==="tactical");
    const rated = Object.entries(store).filter(([k])=>k.endsWith("#"+st)).map(([,v])=>v);
    const choiceRated = rated.filter(v=>v.choice);
    const groups = Object.groupBy(rs.filter(r=>r.parsed?.choice), r=>r.scenario+"/"+r.call_type);
    const flips = Object.values(groups).filter(g=>new Set(g.map(r=>r.parsed.choice)).size>1).length;
    return [st,
      choiceRated.filter(v=>v.choice==="acceptable").length+" / "+choiceRated.length,
      choiceRated.filter(v=>v.choice==="dangerous").length,
      rated.filter(v=>v.better==="yes").length,
      rs.filter(r=>r.checks?.invented_numbers?.length).length+" / "+rs.length,
      rs.filter(r=>r.checks?.choice_valid===false).length,
      flips+" / "+Object.keys(groups).length,
      med(tac.map(r=>r.total_ms))+" / "+Math.max(...tac.map(r=>r.total_ms)),
      med(tac.map(r=>r.ttft_ms)),
      "$"+(rs.reduce((a,r)=>a+r.cost_usd,0)/rs.length).toFixed(5),
      Math.round(rs.reduce((a,r)=>a+(r.usage.output_tokens||0),0)/rs.length),
      Math.round(rs.reduce((a,r)=>a+(r.usage.cache_read_input_tokens||0),0)/rs.length)];
  });
  document.getElementById("summary").innerHTML = "<table><tr>"+["setting","acceptable choices","dangerous","better than Suggested","runs w/ invented numbers (auto)","invalid ids","choice flips across runs","tactical ms median / max","tactical TTFT median","avg $/call","avg output tok","avg cache-read tok"]
    .map(h=>"<th>"+h+"</th>").join("")+"</tr>"+rows.map(r=>"<tr>"+r.map(c=>"<td>"+c+"</td>").join("")+"</tr>").join("")+"</table>"+
    "<p>Pass bar (Haiku default for Alerts): acceptable ≥ 5/6, 0 dangerous, 0 invented numbers, tactical median &lt; 2000 ms and max &lt; 4000 ms, better than Suggested in ≥ 2.</p>";
}
function copyRatings() { navigator.clipboard.writeText(JSON.stringify(store, null, 1)); alert("Ratings copied"); }
</script>`;
writeFileSync("judge.html", html);
console.log(`wrote judge.html (${items.length} items, ${items.reduce((n, i) => n + i.outputs.length, 0)} outputs)`);
