// Browser-side input admission. Acknowledgements mean the complete chunk reached the PTY.
export const MAX_INPUT_BYTES = 1024 * 1024;
export const INPUT_CHUNK_BYTES = 16 * 1024;

export class InputQueue {
    constructor(changed = () => {}) {
        this.changed = changed;
        this.chunks = [];
        this.offset = 0;
        this.bufferedBytes = 0;
        this.inFlightBytes = 0;
        this.connected = false;
        this.ended = false;
        this.warning = "";
        this.send = null;
    }

    notify() {
        this.changed({
            connected: this.connected,
            ended: this.ended,
            paused: this.bufferedBytes >= MAX_INPUT_BYTES,
            warning: this.warning,
        });
    }

    open(send) {
        if (this.ended) return;
        this.connected = true;
        this.send = send;
        this.notify();
        this.pump();
    }

    refuse() {
        this.warning =
            "Input was not sent: the 1 MiB input buffer has insufficient space. Wait for the program to read, or paste less text.";
        this.notify();
        return false;
    }

    text(typed, { binary = false } = {}) {
        // UTF-8 requires at least one byte per UTF-16 code unit. Reject large input
        // before allocating its encoded copy; admission below checks the actual bytes.
        if (typed.length > MAX_INPUT_BYTES - this.bufferedBytes) return this.refuse();
        const bytes = binary
            ? Uint8Array.from(typed, (character) => character.charCodeAt(0))
            : new TextEncoder().encode(typed);
        return this.enqueue(bytes);
    }

    enqueue(bytes) {
        if (!this.connected || this.ended) {
            this.warning ||= "Input was not sent: the terminal is not connected.";
            this.notify();
            return false;
        }
        if (bytes.byteLength > MAX_INPUT_BYTES - this.bufferedBytes) return this.refuse();
        if (bytes.byteLength === 0) return true;
        let copied = 0;
        while (copied < bytes.byteLength) {
            let last = this.chunks.at(-1);
            if (!last || last.used === INPUT_CHUNK_BYTES) {
                last = { bytes: new Uint8Array(INPUT_CHUNK_BYTES), used: 0 };
                this.chunks.push(last);
            }
            const count = Math.min(bytes.byteLength - copied, INPUT_CHUNK_BYTES - last.used);
            last.bytes.set(bytes.subarray(copied, copied + count), last.used);
            last.used += count;
            copied += count;
        }
        this.bufferedBytes += bytes.byteLength;
        this.warning = "";
        this.notify();
        this.pump();
        return true;
    }

    pump() {
        if (!this.connected || this.inFlightBytes || !this.chunks.length) return;
        const chunk = this.chunks[0];
        const end = Math.min(chunk.used, this.offset + INPUT_CHUNK_BYTES);
        this.inFlightBytes = end - this.offset;
        try {
            this.send(chunk.bytes.subarray(this.offset, end));
        } catch {
            this.disconnect();
        }
    }

    acknowledge(bytes) {
        if (!this.inFlightBytes || bytes !== this.inFlightBytes) {
            this.disconnect("Input acknowledgement was invalid. Queued input was not replayed.");
            return false;
        }
        this.offset += bytes;
        this.bufferedBytes -= bytes;
        this.inFlightBytes = 0;
        if (this.offset === this.chunks[0].used) {
            this.chunks.shift();
            this.offset = 0;
        }
        this.notify();
        this.pump();
        return true;
    }

    disconnect(reason = "") {
        if (reason) {
            this.warning = reason;
        } else if (this.bufferedBytes) {
            this.warning =
                "Connection interrupted. Some input may already have reached the program. Queued input was not replayed; check the terminal before typing again.";
        }
        this.connected = false;
        this.send = null;
        this.chunks = [];
        this.offset = 0;
        this.bufferedBytes = 0;
        this.inFlightBytes = 0;
        this.notify();
    }

    finish() {
        this.ended = true;
        this.disconnect(
            this.bufferedBytes ? "The program ended before all queued input was confirmed." : "",
        );
    }
}
