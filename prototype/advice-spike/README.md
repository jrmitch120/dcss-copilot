# PROTOTYPE: advice quality spike (throwaway)

Answers [Advice quality spike: are cheap models good enough and fast enough?](https://github.com/jrmitch120/dcss-copilot/issues/12).
Not production code. Lives only on the `prototype/advice-quality` branch.

Under ADR 0003, the model receives a hand-written Fact Sheet with its Candidate Actions and the
rule-based Suggested Action. It answers with structured output only. Settings:

| id | model | thinking | effort |
|---|---|---|---|
| `haiku-off` | claude-haiku-5-5 | disabled | low |
| `haiku-low` | claude-haiku-5-5 | adaptive | low |
| `sonnet` | claude-sonnet-5-5 | adaptive | high (default) |

Call types:

- `tactical`: for fight scenarios (Alerts, and the Coach card). Returns a Candidate Action and a reason.
- `hints`: one-line nudge at a Decision Point.
- `coach`: Strategic Advice at a Decision Point. Returns a Candidate Action, verdict, reasoning and a Run Plan diff.

## Run

```sh
# one-time: put the key in the keychain
security add-generic-password -s dcss-copilot-spike -a anthropic -w

node run.mjs --dry        # writes preview.md (every prompt, char counts), no API calls
node run.mjs              # 3 runs per cell, sequential, writes results.json
node judge.mjs            # writes judge.html from results.json; open it in a browser
```

`judge.html` hides which setting produced each output until you press Reveal. Ratings
autosave to localStorage. Use "Copy ratings" to export them.
