# Server policy: what's allowed for read-only advisor tools?

Ticket: [#3](https://github.com/jrmitch120/dcss-copilot/issues/3) (map [#1](https://github.com/jrmitch120/dcss-copilot/issues/1)).
Researched 2026-10-13. Sources are primary where they exist: the tournament rules page, the webtiles server source in `crawl/crawl`, the dev-maintained `crawl/qw` README, server front pages, and the crawl wiki. Where no primary source exists, this file says so.

## Answer

- **No public server or the dev team publishes a rule against a read-only Advisor.** I found no written policy on third-party tools that read game state, browser extensions on webtiles, or LLM advice. None of the server front pages I checked (CAO, CXC, CNC, crawl.develz.org) has a rules or terms page. The only written rules are the tournament's, and they target things that give an unfair competitive edge.
- **The tournament rules are an honor system aimed at automation and score inflation.** They name "bots or input macros ... for speedrun points" and alt accounts. A read-only Advisor that never sends input is outside the letter of those rules. Whether LLM advice during a tournament game is "an unfair competitive advantage" is untested and unruled (see Uncertainties).
- **There is precedent for a sanctioned read-only tool on official servers.** The crawl wiki lists the bot `beem` as working on CAO, CBR2, CDI, CUE, CNC, CPO and CXC. It joins players' webtiles games as a spectator and answers game-knowledge queries in chat.
- **The firm limit is automation and server load, not reading.** The dev-maintained qw README says: "Please don't try to run qw on an official server. Misconfigured (or even well-configured) bots can eat up server resources." The Advisor must never send game input, which the glossary already requires. It should also keep a light connection footprint.

## Findings

### 1. Tournament rules (the only written conduct rules)

The [0.34 tournament page](https://crawl.develz.org/tournament/0.34/) has a Conduct section. Quoted:

> Please do not do anything that would give you an unfair competitive advantage over other players or clans. This includes things like scumming crash-on-demand bugs, using alt accounts to inflate the score of your main account, or using bots or input macros on your account to gain an advantage for speedrun points ... just remember that the objective here is to have fun.
>
> We generally do not monitor games or RC files and hold players to the honor system. In extreme cases that come to our attention, we may, at our discretion, disqualify users and remove their games from the scoring database. Ignorance and negligence are not excuses for poor behavior.
>
> Misconduct will result in disqualification without recourse, with any relevant games being deleted from the scoring database.

The page does not mention third-party tools, help from other people or programs, or spoilers. Tournaments run for about 16 days per release (0.34: 6 to 22 February). Past tournaments are listed at https://crawl.develz.org/tournament/.

**Implication:** The ban is on *input* automation ("bots or input macros"). An Advisor that never sends keys does not match that wording. But "anything that would give you an unfair competitive advantage" is a broad catch-all, applied at organiser discretion.

### 2. Server operators' published rules

- **CAO** (https://crawl.akrasiac.org/): no rules or terms. Contact: "#crawl on libera.chat ... The server administrator's nick is rax; advil, gammafunk, |amethyst, and johnstein also help out."
- **CXC** (https://crawl.xtahua.com/): no rules. "If you have any questions, you can contact me on Discord." It has an account-hold mode: "community features and some game modes may be restricted, and games will not be visible to other players" until an admin approves the account.
- **CNC** (https://crawl.nemelex.cards/): no rules; same account-hold text.
- **crawl.develz.org** (https://crawl.develz.org/, `play.htm`): no rules. Links to community channels (#crawl on Libera, Roguelikes Discord #dcss, Tavern forum at https://tavern.dcss.io/).
- CBRO redirects to `:8443`. CDI, CUE and CPO were not checked individually. All of them run the same `crawl/crawl` webtiles server code (section 4).

**Implication:** There is no ToS to comply with. Policy is set informally by admins, who are reachable on IRC and Discord. Asking them before launch is cheap and the right move (see Recommendations).

### 3. Dev-team statements on bots and tools

- **qw** (the dev-org bot that plays DCSS), README at https://github.com/crawl/qw (commit `8698adc`, 2024-07-15):
  > qw requires more memory and cpu than what's allowed for official online servers, so you will need to run your own server. Please don't try to run qw on an official server. Misconfigured (or even well-configured) bots can eat up server resources.

  The older repo (https://github.com/elliptic/qw) also says: "please don't run qw on the same account that you use for your own personal games!" It also asks users to register bot accounts with Sequell so their games can be filtered out of stats.
  The objection is about resource use and keeping bot games out of human stats. It is not about reading state.
- **Knowledge bots in webtiles chat** ([crawl wiki: Guide to the DCSS knowledge Bots](https://github.com/crawl/crawl/wiki/Guide-to-the-DCSS-knowledge-Bots)):
  > If you play on official servers CAO, CBR2, CDI, CUE, CNC, CPO, or CXC, you can subscribe to beem.

  beem watches subscribed players' games and answers LearnDB and monster queries in chat. Etiquette from the same page: "Please limit bot queries in the DCSS channels to things you'd like to discuss with the channel." Players can use `!player-only on` when "spectators are spamming too many DCSS bot commands."
  This is the closest precedent to an Advisor: an automated spectator that gives players Information during play, accepted on all the official servers.
- **LLM and AI tools:** I found no dev statement. Existing projects all run against **local** webtiles servers:
  - [nkhoit/dcss-ai](https://github.com/nkhoit/dcss-ai): default `ws://localhost:8080/socket`.
  - [dtdannen/dcss-ai-wrapper](https://github.com/dtdannen/dcss-ai-wrapper): runs a Docker webserver.
  - [diannt/crawl_AI](https://github.com/diannt/crawl_AI): an "AI companion" fork that analyses game state for advice, built into a modified game rather than played on public servers.
- **Browser extensions and userscripts on webtiles:** I found no statement either way.

### 4. What the webtiles server code enforces

Source: `crawl/crawl` master at `28a137d` (2026-10-13), `crawl-ref/source/webserver/`.

- **Connection cap:** `max_connections` (default 100, `webtiles/config.py`). New sockets over the cap get "The maximum number of connections has been reached, sorry :(" (`ws_handler.py`). Each Advisor connection uses one of the server's slots.
- **No per-client rate limit** on reads in the server code. The only published pacing guidance is third-party. The dcss-api docs (`dcss-api-docs/connection.qmd` in https://github.com/EricFecteau/dcss-api) say:
  > When connecting to a public server, limit your play speed to a maximum of one sent command per 100 milliseconds. Follow any other restrictions from the server (e.g. no bot chat interaction).

  This is not a server rule. It is the only stated convention.
- **Spectating:**
  - Anonymous spectating is allowed by default (`allow_anon_spectate: True`), but servers can turn it off.
  - Accounts on hold or restricted cannot spectate: "Your account is restricted; spectating unavailable."
- **Players control who watches.** The chat commands (`process_handler.py`) let a player run `/kick <name>` and `/block <name>`. `/block [anon]` "disables anonymous spectating" and `/block [all]` "disables spectating".
  However, `is_blocked` lets the player's own username through: `if "[all]" in self.blocked: return username != self.username`. So a connection logged in as the player can still watch their own game when spectating is blocked.
- **Spectators are visible.** Named watchers appear in the spectator list, and anonymous ones show as a count ("N Anon").
- **Idle timeouts:**
  - `max_idle_time` 5h while playing.
  - `max_lobby_idle_time` 3h in the lobby.
- **Bot account flag:** `DGLACCT_BOT` exists, with `bot_accounts: False` by default. `wtutil.py` warns "Bot accounts are not enabled on this server". The comment says "may not be in use", so there is no live bot-account process to use.

## Limits to design around

1. **Never send input.** This is already a glossary invariant. It is also what separates the Advisor from the "bots or input macros" that the tournament rules and qw's README warn about.
2. **Light footprint on official servers:**
   - Use one websocket per watched game.
   - Do not poll the lobby aggressively. The server already pushes lobby updates every `lobby_update_rate` (2s).
   - Reconnect with backoff.
   - Never send chat.
   - Keep any webtiles messages the Advisor itself sends to a minimum, and never more than the community convention of one per 100ms.
3. **Spectating identity.** Watching as Anon or under a separate account is visible to the player and other watchers, and the player's `/block [anon]` or `/block [all]` stops it. Watching as the player's own account gets past blocks but means handling the player's credentials. That decision belongs with the Game Source design (ticket #8, blocked by this one).
4. **Do not post into webtiles chat.** beem's etiquette and the dcss-api guidance ("no bot chat interaction") both point this way. Advice belongs in the Advisor's own UI.
5. **Tournament mode.** The rules are honor-system and discretionary. Two options:
   - Give users a clear notice that using the Advisor in tournament games is at their own risk.
   - Add an optional "tournament mode" that limits output to Information (the kind of thing already available from LearnDB and beem) and turns off Tactical and Strategic Advice.
6. **Prefer local webtiles where possible.** It has no server-load or etiquette concerns at all, and every existing AI project uses it.

## Uncertainties and gaps

- **No ruling on LLM advice in tournaments.** The rules don't mention it. Organisers could treat it as an "unfair competitive advantage." This is the biggest open risk. Only an explicit statement from the tournament organisers or devs can settle it.
- **Unwritten admin policy.** Admins may have unpublished preferences on automated spectators, such as wanting a recognisable account name or a heads-up. I found no written source.
- **Browser extensions** that read the webtiles DOM or socket in the player's own browser: no statement found. Technically they add no server load at all.
- **Not checked:** CDI, CUE, CPO and CBRO pages individually, and Tavern forum threads (search found nothing relevant).

## Recommendations

- Before launch, ask on #crawl (Libera) or Roguelikes Discord #dcss. Cover two things:
  - Server admins: is a read-only spectator client acceptable, and does it need a named account?
  - Tournament organisers: is LLM advice allowed in tournament games?
  Record the answers here.
- Treat local webtiles and in-browser reading as the lowest-risk Game Sources, and remote spectating on official servers as acceptable with a light footprint.
