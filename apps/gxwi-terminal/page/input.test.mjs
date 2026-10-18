import assert from "node:assert/strict";
import test from "node:test";
import { INPUT_CHUNK_BYTES, InputQueue, MAX_INPUT_BYTES } from "./input.mjs";

test("admission counts in-flight bytes and refuses an entire overflowing paste", () => {
    const sent = [];
    const queue = new InputQueue();
    queue.open((bytes) => sent.push(bytes));
    assert.equal(queue.enqueue(new Uint8Array(MAX_INPUT_BYTES)), true);
    assert.equal(queue.bufferedBytes, MAX_INPUT_BYTES);
    assert.equal(sent.length, 1);
    assert.equal(sent[0].length, INPUT_CHUNK_BYTES);
    assert.equal(queue.text("extra"), false);
    assert.equal(queue.bufferedBytes, MAX_INPUT_BYTES);
    assert.match(queue.warning, /not sent/);
    queue.acknowledge(INPUT_CHUNK_BYTES);
    assert.equal(queue.bufferedBytes, MAX_INPUT_BYTES - INPUT_CHUNK_BYTES);
    assert.equal(sent.length, 2);
});

test("accepted bytes and later keystrokes keep order across chunk acknowledgements", () => {
    const sent = [];
    const queue = new InputQueue();
    queue.open((bytes) => sent.push(bytes.slice()));
    const paste = new Uint8Array(INPUT_CHUNK_BYTES + 3).fill(65);
    queue.enqueue(paste);
    queue.text("tail");
    assert.equal(sent.length, 1);
    queue.acknowledge(INPUT_CHUNK_BYTES);
    assert.equal(new TextDecoder().decode(sent[1]), "AAAtail");
    queue.acknowledge(7);
    assert.equal(queue.bufferedBytes, 0);
    assert.equal(queue.inFlightBytes, 0);
});

test("UTF-8 byte admission accounts for multibyte characters", () => {
    const queue = new InputQueue();
    queue.open(() => {});
    assert.equal(queue.text("é".repeat(MAX_INPUT_BYTES / 2 + 1)), false);
    assert.equal(queue.bufferedBytes, 0);
});

test("disconnect is visible and uncertain bytes are never replayed on reconnect", () => {
    const sent = [];
    const queue = new InputQueue();
    queue.open((bytes) => sent.push(bytes.slice()));
    queue.text("command\n");
    queue.text("later\n");
    queue.disconnect();
    assert.match(queue.warning, /not replayed/);
    assert.equal(queue.text("offline"), false);
    queue.open((bytes) => sent.push(bytes.slice()));
    assert.equal(sent.length, 1);
    assert.match(queue.warning, /not replayed/);
    queue.text("new");
    assert.equal(new TextDecoder().decode(sent[1]), "new");
});

test("invalid acknowledgements cannot advance or replay input", () => {
    const queue = new InputQueue();
    queue.open(() => {});
    queue.text("first");
    assert.equal(queue.acknowledge(4), false);
    assert.equal(queue.connected, false);
    assert.equal(queue.bufferedBytes, 0);
    assert.match(queue.warning, /invalid/);
});

test("ending a program releases buffered input and refuses reconnection", () => {
    const queue = new InputQueue();
    queue.open(() => {});
    queue.text("waiting");
    queue.finish();
    assert.equal(queue.bufferedBytes, 0);
    assert.match(queue.warning, /before all queued input was confirmed/);
    queue.open(() => assert.fail("must not replay"));
    assert.equal(queue.connected, false);
});

test("single-byte keystrokes are coalesced into bounded storage", () => {
    const queue = new InputQueue();
    queue.open(() => {});
    for (let index = 0; index < MAX_INPUT_BYTES; index++) queue.enqueue(new Uint8Array([120]));
    assert.equal(queue.bufferedBytes, MAX_INPUT_BYTES);
    assert.equal(queue.chunks.length, MAX_INPUT_BYTES / INPUT_CHUNK_BYTES);
});
