// Draws the terminal, and passes on what is typed.
//
// The terminal is the process's. This page has an emulator (xterm.js) to
// draw it with and nothing to keep: when the page comes, or comes back, it
// is sent what prints the terminal as it stands, and then whatever is
// printed. `src/page.rs` sets out the wire.
//
// THE SIZE is the terminal's and there is one of it, however many pages are
// drawing it. A page says what it has room for when that changes, when the
// keyboard comes to it, and when it arrives, and draws the terminal as big
// as the process then says it is, which another page may since have changed.
//
// COPYING is the desktop's to do, as for every surface: Ctrl+Shift+C has
// the desktop's page copy what is selected. Pasting is the browser's:
// Ctrl+Shift+V and Shift+Insert are left to it, and the emulator hears what
// was pasted. Ctrl+C and Ctrl+V are the terminal's own, as they are in any
// terminal.

import { Terminal } from "./vendor/xterm.mjs";
import { FitAddon } from "./vendor/addon-fit.mjs";

const room = document.getElementById("room");
const said = document.getElementById("said");

/** A colour of the theme's, by the name of its custom property. */
const colour = (name) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

function theme() {
    const numbered = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"];
    const named = { background: colour("--bg-base"), foreground: colour("--text-body"), cursor: colour("--accent-solid"), cursorAccent: colour("--bg-base"), selectionBackground: colour("--term-selection") };
    for (const name of numbered) {
        named[name] = colour(`--term-${name}`);
        named[`bright${name[0].toUpperCase()}${name.slice(1)}`] = colour(`--term-bright-${name}`);
    }
    return named;
}

const term = new Terminal({
    fontFamily: colour("--font-mono") || "ui-monospace, monospace",
    fontSize: 14,
    cursorBlink: true,
    scrollback: 5000,
    theme: theme(),
});
const fit = new FitAddon();
term.loadAddon(fit);
term.open(document.getElementById("terminal"));
matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => (term.options.theme = theme()));

function say(what) {
    said.textContent = what ?? "";
    said.hidden = !what;
}

// ---- the wire

let socket = null;
let ended = false;
/** Whether what comes next prints the terminal as it stands. */
let afresh = false;

const encoder = new TextEncoder();
const send = (what) => socket?.readyState === WebSocket.OPEN && socket.send(what);

function connect() {
    const address = new URL("socket", location.href);
    address.protocol = address.protocol === "https:" ? "wss:" : "ws:";
    const made = new WebSocket(address);
    made.binaryType = "arraybuffer";
    socket = made;
    made.onopen = () => say(null);
    made.onmessage = (message) => {
        if (typeof message.data !== "string") {
            term.write(new Uint8Array(message.data), afresh ? ask : undefined);
            afresh = false;
            return;
        }
        const heard = JSON.parse(message.data);
        switch (heard.type) {
            case "stands":
                term.reset();
                term.resize(heard.cols, heard.rows);
                afresh = true;
                break;
            case "size":
                term.resize(heard.cols, heard.rows);
                break;
            case "ended":
                ended = true;
                say("Ended");
                break;
        }
    };
    made.onclose = () => {
        if (socket !== made || ended) return;
        say("Not reaching the terminal. Trying again.");
        setTimeout(connect, 1000);
    };
}

term.onData((typed) => send(encoder.encode(typed)));
// What is not text: a mouse's reports, in the older way of making them.
term.onBinary((typed) => send(Uint8Array.from(typed, (c) => c.charCodeAt(0))));

// ---- the size

/** Says what this page has room for, if it has any: one out of sight has none. */
function ask() {
    const box = room.getBoundingClientRect();
    if (box.width < 40 || box.height < 20 || document.visibilityState !== "visible") return;
    const room_for = fit.proposeDimensions();
    if (!room_for || !(room_for.cols >= 2) || !(room_for.rows >= 2)) return;
    if (room_for.cols === term.cols && room_for.rows === term.rows) return;
    send(JSON.stringify({ type: "size", rows: room_for.rows, cols: room_for.cols }));
}

let asking = 0;
new ResizeObserver(() => {
    clearTimeout(asking);
    asking = setTimeout(ask, 60);
}).observe(room);

// ---- the keyboard

window.addEventListener("focus", () => {
    term.focus();
    ask();
});

function copy() {
    const text = term.getSelection();
    if (text && window.parent !== window) window.parent.postMessage({ fx: "copy", text }, location.origin);
}

term.attachCustomKeyEventHandler((event) => {
    const pasting = (event.ctrlKey && event.shiftKey && event.code === "KeyV") || (event.shiftKey && !event.ctrlKey && event.key === "Insert");
    if (pasting) return false;
    if (event.ctrlKey && event.shiftKey && event.code === "KeyC") {
        if (event.type === "keydown") copy();
        // Or the browser does what it does with this chord, which is not copying.
        event.preventDefault();
        return false;
    }
    return true;
});

connect();
if (document.hasFocus()) term.focus();
