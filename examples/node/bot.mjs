// An echo bot on Node.js or Bun: it accepts every request, greets each new friend, and answers
// each message with its text. It reaches relays over direct TCP (no tunnel needed) and keeps
// its store, sealed under BOT_KEY, as ./bot.snapshot and the changes saved after it in
// ./bot.changes. Build with scripts/build-js.sh, then run `node examples/node/bot.mjs` or
// `bun examples/node/bot.mjs`.
import { existsSync, mkdirSync, readdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { Client } from "simplex-client";

const FILE = "./bot.snapshot";
const DIR = "./bot.changes"; // one file per saveChanges(), numbered in order
const COMPACT_AFTER = 1000; // changes kept before the next whole save

// Written to a side file first, so a crash mid-write never leaves half a file.
function write(path, bytes) {
  writeFileSync(`${path}.tmp`, bytes);
  renameSync(`${path}.tmp`, path);
}

function saved() {
  if (!existsSync(FILE)) return {};
  const names = existsSync(DIR) ? readdirSync(DIR).filter((f) => /^\d+$/.test(f)) : [];
  names.sort((a, b) => a - b);
  return {
    snapshot: new Uint8Array(readFileSync(FILE)),
    changes: names.map((f) => new Uint8Array(readFileSync(`${DIR}/${f}`))),
  };
}

const bot = await Client.start({
  name: "Echo Bot",
  key: process.env.BOT_KEY ?? "change this passphrase",
  ...saved(),
});

// A whole save, then an empty log. A crash between the two leaves older changes behind, which
// the next start skips: they belong to the snapshot this one replaced.
let next = 0;
function saveWhole() {
  write(FILE, bot.save());
  rmSync(DIR, { recursive: true, force: true });
  mkdirSync(DIR);
  next = 0;
}
saveWhole();

// Each change costs what changed, not the size of the store.
bot.on("change", () => {
  try {
    write(`${DIR}/${next}`, bot.saveChanges());
    next += 1;
    if (next >= COMPACT_AFTER) saveWhole();
  } catch {
    saveWhole(); // a change that was not kept would break the chain, so start a new one
  }
});
bot.on("ready", (me) => console.log(`Logged in as ${me.name}`));
bot.on("request", (request) => request.accept());
bot.on("friendAdd", (friend) => friend.send("Hi! I say back what you write."));
bot.on("message", async (message) => {
  if (message.mine || !message.text) return;
  await message.reply(message.text);
});
bot.on("offline", () => console.log("offline"));
bot.on("online", () => console.log("online"));
console.log("My address:", await bot.createAddress());

process.on("SIGINT", () => {
  bot.close();
  process.exit(0);
});
