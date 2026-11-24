// An echo bot on Node.js or Bun: it accepts every request, greets each new friend, and answers
// each message with its text. It reaches relays over direct TCP (no tunnel needed) and keeps
// its store, sealed under BOT_KEY, in ./bot.snapshot. Build with scripts/build-js.sh, then
// run `node examples/node/bot.mjs` or `bun examples/node/bot.mjs`.
import { existsSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { Client } from "simplex-client";

const FILE = "./bot.snapshot";
const snapshot = existsSync(FILE) ? readFileSync(FILE) : undefined;
const bot = await Client.start({
  name: "Echo Bot",
  key: process.env.BOT_KEY ?? "change this passphrase",
  snapshot: snapshot && new Uint8Array(snapshot),
});

// Written to a side file first, so a crash mid-write never leaves half a store.
function save() {
  writeFileSync(`${FILE}.tmp`, bot.save());
  renameSync(`${FILE}.tmp`, FILE);
}

let timer;
bot.on("change", () => {
  clearTimeout(timer);
  timer = setTimeout(save, 1000);
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
  save();
  bot.close();
  process.exit(0);
});
