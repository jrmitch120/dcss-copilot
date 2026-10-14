# Deterministic engine computes, LLM only chooses and explains

Every number the Advisor shows or says comes from a deterministic engine in the Rust core: chance of death, worst-case hit, Threat Level, fight outcomes, Ways Out, item comparisons, failure rates and Great Find tiers. Together they form the Fact Sheet. The LLM never produces numbers. For Tactical Advice and Alerts, it uses structured output to choose from options the engine has enumerated as legal and available, and adds a short reason. Anything outside that list is dropped. Only deterministic detectors decide when the LLM is called. We chose this because cheap models are bad at arithmetic, and a confidently wrong "you'll survive" is the worst failure the Advisor can have. It also makes Info mode simply the engine without the LLM, keeps calls event-driven so we stay within the cost target, and makes advice testable against recorded sessions.

## Considered Options

- **LLM estimates what the engine doesn't cover** (e.g. "this fight is probably winnable"): more coverage, but the numbers players trust most would be the least reliable ones.
- **Free-text advice checked afterwards for contradictions:** more natural output, but catching "read teleportation" while under -Tele means parsing prose, and anything the check misses reaches the player.
- **Periodically polling the LLM for "anything worth saying?":** catches events the detectors miss, but makes the call rate, and therefore the cost, unbounded, and makes Alerts untestable.

## Consequences

- When the engine can't compute something, the Advisor can't say it. Coverage grows by adding engine features, not by prompting.
- Missing inputs, such as spells not yet read, are marked on the Fact Sheet rather than guessed by the LLM.
- The ask box can use free text, but it gets tools over the engine (lookup, compare, simulate), so "what if" answers still come from engine math.
- The Run Plan is changed by LLM-proposed structured diffs that the engine validates, so Great Find scoring stays deterministic.
