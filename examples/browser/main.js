// A minimal chat page: your address, requests to accept, friends, messages and a send box.
// The store is saved to IndexedDB, sealed under the passphrase, a moment after each change.
import init, { Client, Friend } from "./pkg/simplex_client.js";

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
let current; // the Friend whose messages are shown
let saveTimer;

function saveSoon() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => store(client.save()).catch(console.error), 1000);
}

function showMessage(message) {
  if (!current || message.chat.id !== current.id || !(message.chat instanceof Friend)) return;
  const p = document.createElement("p");
  p.className = message.mine ? "mine" : "";
  // Notices (a friend connected, a call ended) carry no text; show their kind instead.
  p.textContent = message.text || `(${message.raw.body.t})`;
  $("messages").append(p);
  $("messages").scrollTop = $("messages").scrollHeight;
}

async function open(friend) {
  current = friend;
  for (const b of $("friends").children) b.classList.toggle("on", b.dataset.id == friend.id);
  $("messages").replaceChildren();
  for (const message of await friend.history()) showMessage(message);
  friend.markRead();
}

function showFriends() {
  const buttons = [...client.friends.values()].map((friend) => {
    const b = document.createElement("button");
    b.dataset.id = friend.id;
    b.textContent = friend.ready ? friend.name : `${friend.name} (connecting…)`;
    b.classList.toggle("on", friend.id === current?.id);
    b.onclick = () => open(friend);
    return b;
  });
  $("friends").replaceChildren(...buttons);
}

function showRequest(request) {
  const row = document.createElement("div");
  row.className = "row";
  row.append(`${request.name} wants to connect `);
  for (const [label, answer] of [["Accept", "accept"], ["Reject", "reject"]]) {
    const b = document.createElement("button");
    b.textContent = label;
    b.onclick = () => {
      request[answer]();
      row.remove();
    };
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
  client.on("ready", (me) => ($("status").textContent = `Signed in as ${me.name}.`));
  client.on("offline", () => ($("status").textContent = "Offline: no relay reachable."));
  client.on("online", () => ($("status").textContent = "Online."));
  client.on("request", showRequest);
  client.on("message", showMessage);
  // The list changes when a friend is added, removed or finishes connecting.
  client.on("raw", (event) => event.t === "chats" && showFriends());
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
  if (current && $("text").value) current.send($("text").value);
  $("text").value = "";
};
addEventListener("beforeunload", () => client && store(client.save()));
