// The Node build over direct TCP with no relay reachable: it starts, takes requests, acts
// through the objects its events carry, says it is offline, saves and closes cleanly. Run
// `scripts/build-js.sh`, then `node --test tests/js/` or `bun test ./tests/js/`.
import { test } from "node:test";
import assert from "node:assert/strict";
import net from "node:net";
import { Client, Group, Message } from "simplex-client";

// A relay address on a local port that nothing listens on: every connect is refused.
async function deadRelay() {
  const server = net.createServer();
  await new Promise((done) => server.listen(0, "127.0.0.1", done));
  const { port } = server.address();
  await new Promise((done) => server.close(done));
  return `smp://jZxOZYTgS4Q3Mp5r4wJW5ruxuQAPb8y5pa4a1DI2l6Y=@127.0.0.1:${port}`;
}

// The next `name` event that passes `test`, or a rejection after `ms`.
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
  assert.equal(client.user?.name, "Node", "user answers as soon as start resolves");
  assert.equal((await client.once("ready")).name, "Node");

  const club = await client.createGroup("Club");
  assert.ok(club instanceof Group);
  assert.equal(client.groups.get(club.id)?.name, "Club");

  const offline = client.once("offline");
  const invite = client.createInvite();
  await offline;
  await assert.rejects(invite, (e) => e.code === "timeout");

  const bytes = client.save();
  assert.ok(bytes instanceof Uint8Array && bytes.length > 0);
  client.close();
  await assert.rejects(club.send("late"), (e) => e.code === "closed");

  const again = await Client.start({ key: "node-key", snapshot: bytes, relays: [await deadRelay()] });
  assert.equal((await again.once("ready")).name, "Node");
  assert.equal(again.groups.get(club.id)?.name, "Club");
  again.close();
});

test("a group's message is a Message whose reply quotes it in the same group", async () => {
  const client = await Client.start({ key: "k", relays: [await deadRelay()] });
  const club = await client.createGroup("Club");

  const hello = next(client, "message", (m) => m.text === "hello");
  await club.send("hello");
  const message = await hello;
  assert.ok(message instanceof Message);
  assert.ok(message.mine);
  assert.equal(message.author, null, "the user's own message has no other author");
  assert.equal(message.chat.id, club.id);

  const again = next(client, "message", (m) => m.text === "again");
  await message.reply("again");
  const reply = await again;
  assert.equal(reply.replyTo, message.id);
  assert.ok(reply.chat instanceof Group);

  const history = await club.history({ limit: 10 });
  assert.deepEqual(
    history.filter((m) => m.text).map((m) => m.text),
    ["hello", "again"],
  );

  const removed = next(client, "groupRemove");
  await club.leave();
  assert.equal((await removed).id, club.id);
  assert.equal(client.groups.get(club.id)?.membership, "left");
  client.close();
});

test("a browser-style start without a tunnel or TCP is refused with a clear error", async () => {
  await assert.rejects(
    Client.start({ key: "k", transport: "tunnel" }),
    (e) => e.code === "bad_option" && /tunnel/.test(e.detail),
  );
});

test("an unknown privateRouting value is refused with a clear error", async () => {
  await assert.rejects(
    Client.start({ key: "k", transport: "tcp", privateRouting: "sometimes" }),
    (e) => e.code === "bad_option" && /privateRouting/.test(e.detail),
  );
});

test("what breaks one of the official client's limits rejects with the limit's code", async () => {
  const client = await Client.start({ key: "k", relays: [await deadRelay()] });
  const club = await client.createGroup("Club");
  await assert.rejects(club.send("x".repeat(16000)), (e) => e.code === "text_too_long");
  await assert.rejects(client.createGroup("#club"), (e) => e.code === "invalid_name");

  const hello = next(client, "message", (m) => m.text === "hello");
  await club.send("hello");
  const message = await hello;
  await assert.rejects(message.react("🤡"), (e) => e.code === "reaction_not_allowed");
  await message.react("👍");
  client.close();

  await assert.rejects(
    Client.start({ key: "k", name: "J.Doe", relays: [await deadRelay()] }),
    (e) => e.code === "invalid_name",
  );
});
