// The Node build over direct TCP with no relay reachable: it starts, takes requests, says it
// is offline, saves and closes cleanly. Run `scripts/build-js.sh`, then `node --test tests/js/*.test.mjs`.
import { test } from "node:test";
import assert from "node:assert/strict";
import net from "node:net";
import { Client } from "simplex-client";

// A relay address on a local port that nothing listens on: every connect is refused.
async function deadRelay() {
  const server = net.createServer();
  await new Promise((done) => server.listen(0, "127.0.0.1", done));
  const { port } = server.address();
  await new Promise((done) => server.close(done));
  return `smp://jZxOZYTgS4Q3Mp5r4wJW5ruxuQAPb8y5pa4a1DI2l6Y=@127.0.0.1:${port}`;
}

function next(client, name, test = () => true, ms = 5000) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(`no ${name} event`)), ms);
    const f = (v) => {
      if (!test(v)) return;
      clearTimeout(timer);
      client.off(name, f);
      resolve(v);
    };
    client.on(name, f);
  });
}

test("a client over TCP with no relay reachable takes requests and goes offline cleanly", async () => {
  const client = await Client.start({
    name: "Node",
    key: "node-key",
    relays: [await deadRelay()],
    timeout: 1500,
  });
  const ready = await next(client, "ready");
  assert.equal(ready.name, "Node");

  const chats = next(client, "chats", (c) => c.groups.some((g) => g.name === "Club"));
  client.newGroup("Club");
  await chats;

  const offline = next(client, "offline", (v) => v === true);
  const invite = client.createInvite();
  await offline;
  await assert.rejects(invite, (e) => e.code === "timeout");

  const bytes = client.save();
  assert.ok(bytes instanceof Uint8Array && bytes.length > 0);
  client.close();
  assert.throws(() => client.send({ friend: 1 }, "late"), (e) => e.code === "closed");

  const again = await Client.start({ key: "node-key", snapshot: bytes, relays: [await deadRelay()] });
  assert.equal((await next(again, "ready")).name, "Node");
  again.close();
});

test("a browser-style start without a tunnel or TCP is refused with a clear error", async () => {
  await assert.rejects(
    Client.start({ key: "k", transport: "tunnel" }),
    (e) => e.code === "bad_option" && /tunnel/.test(e.detail),
  );
});
