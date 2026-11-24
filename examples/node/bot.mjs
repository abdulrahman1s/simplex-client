// An echo bot on Node.js: it accepts every request and answers each message with its text.
// It reaches relays over direct TCP (no tunnel needed) and keeps its store, sealed under
// BOT_KEY, in ./bot.snapshot. Build with scripts/build-js.sh, then: node examples/node/bot.mjs
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
bot.on("request", (req) => bot.accept(req.id));
bot.on("message", (msg) => {
  if (!msg.mine && msg.text) bot.reply(msg, msg.text);
});
bot.on("offline", (off) => console.log(off ? "offline" : "online"));
console.log("my address:", await bot.createAddress());

process.on("SIGINT", () => {
  save();
  bot.close();
  process.exit(0);
});
