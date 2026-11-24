// A minimal chat page: your address, requests to accept, friends, messages and a send box.
// The store is saved to IndexedDB, sealed under the passphrase, a moment after each change.
import init, { Client } from "./pkg/simplex_client.js";

const $ = (id) => document.getElementById(id);

// IndexedDB: one object store holding the snapshot under one key.
function db() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open("simplex-client", 1);
    req.onupgradeneeded = () => req.result.createObjectStore("snapshots");
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function load() {
  const d = await db();
  return new Promise((resolve, reject) => {
    const req = d.transaction("snapshots").objectStore("snapshots").get("main");
    req.onsuccess = () => resolve(req.result ?? undefined);
    req.onerror = () => reject(req.error);
  });
}

async function store(bytes) {
  const d = await db();
  return new Promise((resolve, reject) => {
    const tx = d.transaction("snapshots", "readwrite");
    tx.objectStore("snapshots").put(bytes, "main");
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

let client;
let current; // the friend whose messages are shown
let saveTimer;

function saveSoon() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => store(client.save()).catch(console.error), 1000);
}

function showMessage(m) {
  if (!current || m.chat.t !== "direct" || m.chat.c !== current) return;
  const p = document.createElement("p");
  p.className = m.mine ? "mine" : "";
  p.textContent = m.text;
  $("messages").append(p);
  $("messages").scrollTop = $("messages").scrollHeight;
}

async function open(friend) {
  current = friend.id;
  for (const b of $("friends").children) b.classList.toggle("on", b.dataset.id == friend.id);
  $("messages").replaceChildren();
  const chat = { friend: friend.id };
  for (const m of await client.history(chat)) showMessage(m);
  client.markRead(chat);
}

function showChats({ friends }) {
  $("friends").replaceChildren(
    ...friends.map((f) => {
      const b = document.createElement("button");
      b.dataset.id = f.id;
      b.textContent = f.ready ? f.name : `${f.name} (connecting…)`;
      b.classList.toggle("on", f.id === current);
      b.onclick = () => open(f);
      return b;
    }),
  );
}

function showRequest(req) {
  const row = document.createElement("div");
  row.className = "row";
  row.append(`${req.name} wants to connect `);
  for (const [label, act] of [["Accept", "accept"], ["Reject", "reject"]]) {
    const b = document.createElement("button");
    b.textContent = label;
    b.onclick = () => { client[act](req.id); row.remove(); };
    row.append(b);
  }
  $("requests").append(row);
}

async function start(e) {
  e.preventDefault();
  localStorage.setItem("tunnel", $("tunnel").value);
  localStorage.setItem("name", $("name").value);
  $("status").textContent = "Starting…";
  await init();
  try {
    client = await Client.start({
      name: $("name").value,
      key: $("key").value,
      tunnel: $("tunnel").value,
      snapshot: await load(),
    });
  } catch (err) {
    $("status").textContent = err.code === "wrong_key" ? "Wrong passphrase." : `Failed: ${err.detail}`;
    return;
  }
  $("start").hidden = true;
  $("app").hidden = false;
  client.on("change", saveSoon);
  client.on("ready", (p) => ($("status").textContent = `Signed in as ${p.name}.`));
  client.on("offline", (off) => ($("status").textContent = off ? "Offline: no relay reachable." : "Online."));
  client.on("chats", showChats);
  client.on("request", showRequest);
  client.on("message", showMessage);
  client.createAddress().then(
    (link) => ($("address").textContent = link),
    (err) => ($("address").textContent = `not made: ${err.detail}`),
  );
}

$("tunnel").value = localStorage.getItem("tunnel") ?? "";
$("name").value = localStorage.getItem("name") ?? "";
$("start").onsubmit = start;
$("copy").onclick = () => navigator.clipboard.writeText($("address").textContent);
$("connect").onsubmit = (e) => {
  e.preventDefault();
  if ($("link").value.trim()) client.connect($("link").value);
  $("link").value = "";
};
$("send").onsubmit = (e) => {
  e.preventDefault();
  if (current && $("text").value) client.send({ friend: current }, $("text").value);
  $("text").value = "";
};
addEventListener("beforeunload", () => client && store(client.save()));
