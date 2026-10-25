import { describe, expect, it } from "vitest";
import { createAppLink, type LinkStatus, type Socket } from "../lib/app-link";
import { type Comm, wrapComm } from "../lib/page-relay";
import type { PanelFrame } from "../lib/protocol";

const ORIGIN = "https://crawl.example.org";
const TAB = 7;

class FakeSocket implements Socket {
  sent: unknown[] = [];
  closed = false;
  onopen: (() => void) | null = null;
  onmessage: ((event: { data: unknown }) => void) | null = null;
  onclose: (() => void) | null = null;

  send(data: string): void {
    this.sent.push(JSON.parse(data));
  }

  close(): void {
    this.closed = true;
    this.onclose?.();
  }

  /** The app's side: open the connection. */
  open(): void {
    this.onopen?.();
  }

  /** The app's side: send a frame to the extension. */
  receive(frame: unknown): void {
    this.onmessage?.({ data: JSON.stringify(frame) });
  }

  envelopes(): unknown[] {
    return this.sent.filter((f) => (f as { type: string }).type === "envelope");
  }
}

/** A webtiles page's `comm` module, with the extension's relay wrapped around it. */
function relayHarness(options: { bufferLimitBytes?: number } = {}) {
  const handledByPage: unknown[] = [];
  const comm: Comm = {
    handle_message(msg) {
      handledByPage.push(msg);
    },
  };
  const sockets: FakeSocket[] = [];
  const retries: (() => void)[] = [];
  const statuses: LinkStatus[] = [];
  const toSidePanels: PanelFrame[] = [];
  let clock = 1_000;

  const link = createAppLink({
    connect: () => {
      const socket = new FakeSocket();
      sockets.push(socket);
      return socket;
    },
    requestReplay: () => page.replay(),
    retryLater: (reconnect) => retries.push(reconnect),
    status: (status) => statuses.push(status),
    toSidePanels: (frame) => toSidePanels.push(frame),
  });
  const page = wrapComm(comm, {
    origin: ORIGIN,
    now: () => clock,
    report: (report) => link.report(TAB, report),
    bufferLimitBytes: options.bufferLimitBytes,
  });

  return {
    handledByPage,
    sockets,
    retries,
    statuses,
    toSidePanels,
    link,
    /** The game server sends the page a message. */
    serverSends(msg: unknown) {
      clock += 10;
      comm.handle_message(msg);
    },
    /** The app accepts a new connection and welcomes the browser. */
    connectAndWelcome(): FakeSocket {
      link.start();
      const socket = sockets.at(-1) as FakeSocket;
      socket.open();
      socket.receive({ type: "welcome", protocol: 4 });
      return socket;
    },
  };
}

describe("extension relay", () => {
  it("opens every connection with a hello carrying the relay protocol version", () => {
    const relay = relayHarness();
    relay.link.start();
    const socket = relay.sockets[0] as FakeSocket;
    socket.open();

    expect(socket.sent).toEqual([{ type: "hello", protocol: 4 }]);
  });

  it("forwards each message unchanged in an envelope once the app welcomes the browser", () => {
    const relay = relayHarness();
    const socket = relay.connectAndWelcome();

    relay.serverSends({ msg: "version", text: "Dungeon Crawl Stone Soup 0.34.1" });
    relay.serverSends({ msg: "player", name: "Mira", hp: 18 });

    expect(socket.envelopes()).toEqual([
      {
        type: "envelope",
        envelope: {
          origin: ORIGIN,
          tab: TAB,
          ts: 1_010,
          gameVersion: "Dungeon Crawl Stone Soup 0.34.1",
          msg: { msg: "version", text: "Dungeon Crawl Stone Soup 0.34.1" },
        },
      },
      {
        type: "envelope",
        envelope: {
          origin: ORIGIN,
          tab: TAB,
          ts: 1_020,
          gameVersion: "Dungeon Crawl Stone Soup 0.34.1",
          msg: { msg: "player", name: "Mira", hp: 18 },
        },
      },
    ]);
    expect(relay.handledByPage).toEqual([
      { msg: "version", text: "Dungeon Crawl Stone Soup 0.34.1" },
      { msg: "player", name: "Mira", hp: 18 },
    ]);
  });

  it("replays everything the page saw before the app connected, in order, then goes live", () => {
    const relay = relayHarness();
    relay.serverSends({ msg: "game_started" });
    relay.serverSends({ msg: "player", name: "Mira" });

    const socket = relay.connectAndWelcome();
    relay.serverSends({ msg: "player", hp: 17 });

    expect(socket.envelopes().map(messageOf)).toEqual([
      { msg: "game_started" },
      { msg: "player", name: "Mira" },
      { msg: "player", hp: 17 },
    ]);
  });

  it("replays messages as the server sent them, even if the page changed them after handling", () => {
    const relay = relayHarness();
    const msg = { msg: "player", inv: { 0: { name: "a +0 club" } } };
    relay.serverSends(msg);
    msg.inv[0].name = "changed by the page";

    const socket = relay.connectAndWelcome();

    expect(socket.envelopes().map(messageOf)).toEqual([
      { msg: "player", inv: { 0: { name: "a +0 club" } } },
    ]);
  });

  it("replays the whole buffer again when the app reconnects", () => {
    const relay = relayHarness();
    relay.serverSends({ msg: "player", name: "Mira" });
    relay.connectAndWelcome().close();
    relay.serverSends({ msg: "player", hp: 17 });

    const second = relay.connectAndWelcome();

    expect(second.envelopes().map(messageOf)).toEqual([
      { msg: "player", name: "Mira" },
      { msg: "player", hp: 17 },
    ]);
  });

  it("skips multi wrappers, whose children reach handle_message on their own", () => {
    const relay = relayHarness();
    const socket = relay.connectAndWelcome();

    relay.serverSends({ msg: "multi", msgs: [{ msg: "player", hp: 17 }] });
    relay.serverSends({ msg: "player", hp: 17 });

    expect(socket.envelopes().map(messageOf)).toEqual([{ msg: "player", hp: 17 }]);
  });

  it("signals an overflowed buffer instead of replaying a partial history", () => {
    const relay = relayHarness({ bufferLimitBytes: 60 });
    relay.serverSends({ msg: "player", name: "Mira", species: "Minotaur" });
    relay.serverSends({ msg: "player", hp: 17, hp_max: 18 });

    const socket = relay.connectAndWelcome();
    relay.serverSends({ msg: "player", hp: 16 });

    expect(socket.sent.slice(1)).toEqual([
      { type: "overflow", origin: ORIGIN, tab: TAB },
      {
        type: "envelope",
        envelope: {
          origin: ORIGIN,
          tab: TAB,
          ts: 1_030,
          gameVersion: null,
          msg: { msg: "player", hp: 16 },
        },
      },
    ]);
  });
});

describe("handshake", () => {
  it("holds envelopes while the player is asked to allow this browser", () => {
    const relay = relayHarness();
    relay.serverSends({ msg: "player", name: "Mira" });
    relay.link.start();
    const socket = relay.sockets[0] as FakeSocket;
    socket.open();

    socket.receive({ type: "awaiting-approval" });
    relay.serverSends({ msg: "player", hp: 17 });
    expect(socket.envelopes()).toEqual([]);

    socket.receive({ type: "welcome", protocol: 4 });
    expect(socket.envelopes().map(messageOf)).toEqual([
      { msg: "player", name: "Mira" },
      { msg: "player", hp: 17 },
    ]);
  });

  it("closes a refused connection and tries again later", () => {
    const relay = relayHarness();
    relay.link.start();
    const socket = relay.sockets[0] as FakeSocket;
    socket.open();

    socket.receive({ type: "refused", reason: "browser-denied" });

    expect(socket.closed).toBe(true);
    expect(relay.retries).toHaveLength(1);
    relay.retries[0]?.();
    expect(relay.sockets).toHaveLength(2);
  });

  it("tries again later when the app isn't running", () => {
    const relay = relayHarness();
    relay.link.start();

    (relay.sockets[0] as FakeSocket).close();

    expect(relay.retries).toHaveLength(1);
  });
});

describe("side panels", () => {
  const sidePanelFrames = (socket: FakeSocket) =>
    socket.sent.filter((f) =>
      /^(side-panel-opened|panel-command)$/.test((f as { type: string }).type),
    );

  it("tells the app as a side panel opens, and on every connection while one is open", () => {
    const relay = relayHarness();
    relay.link.sidePanel(true);

    const first = relay.connectAndWelcome();
    relay.link.sidePanel(false);
    relay.link.sidePanel(true);
    first.close();
    const second = relay.connectAndWelcome();

    expect(sidePanelFrames(first)).toEqual([
      { type: "side-panel-opened" },
      { type: "side-panel-opened" },
    ]);
    expect(sidePanelFrames(second)).toEqual([{ type: "side-panel-opened" }]);
  });

  it("passes the side panels' commands to the app, and the app's views and answers back", () => {
    const relay = relayHarness();
    const socket = relay.connectAndWelcome();

    relay.link.panelCommand(3, { kind: "set-appearance", appearance: "glyphs" });
    relay.link.panelCommand(4, { kind: "set-assistance", assistance: "coach" });
    relay.link.panelCommand(5, { kind: "answer-same-game", same: false });
    socket.receive({ type: "view", view: { appVersion: "1.2.3" } });
    socket.receive({ type: "panel-command-done", id: 3, error: null });

    expect(sidePanelFrames(socket)).toEqual([
      {
        type: "panel-command",
        id: 3,
        command: { kind: "set-appearance", appearance: "glyphs" },
      },
      {
        type: "panel-command",
        id: 4,
        command: { kind: "set-assistance", assistance: "coach" },
      },
      {
        type: "panel-command",
        id: 5,
        command: { kind: "answer-same-game", same: false },
      },
    ]);
    expect(relay.toSidePanels).toEqual([
      { type: "view", view: { appVersion: "1.2.3" } },
      { type: "panel-command-done", id: 3, error: null },
    ]);
  });

  it("tells the side panels where the browser stands with the app", () => {
    const relay = relayHarness();
    relay.link.start();
    const asking = relay.sockets[0] as FakeSocket;
    asking.open();
    asking.receive({ type: "awaiting-approval" });
    asking.receive({ type: "welcome", protocol: 4 });
    asking.close();
    relay.retries[0]?.();
    const refused = relay.sockets[1] as FakeSocket;
    refused.open();
    refused.receive({ type: "refused", reason: "browser-denied" });

    expect(relay.statuses).toEqual(["awaiting-approval", "connected", "no-app", "browser-denied"]);
  });
});

function messageOf(frame: unknown): unknown {
  return (frame as { envelope: { msg: unknown } }).envelope.msg;
}
