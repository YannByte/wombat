import type { Readable } from "node:stream";
import { CoreError } from "@wombat/client";
import { INPUT_BYTES, INPUT_TIMEOUT_MS } from "./agent-limits.js";

export function readJsonInput(input: Readable, signal: AbortSignal, limit = INPUT_BYTES): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    let bytes = 0,
      finished = false;
    const finish = (error?: unknown) => {
      if (finished) return;
      finished = true;
      clearTimeout(timer);
      input.off("data", data);
      input.off("end", end);
      input.off("error", fail);
      input.pause();
      signal.removeEventListener("abort", abort);
      if (error) {
        reject(error);
        return;
      }
      try {
        resolve(
          JSON.parse(
            new TextDecoder("utf-8", { fatal: true }).decode(
              Buffer.concat(chunks),
            ),
          ),
        );
      } catch {
        reject(new CoreError("INVALID_ARGUMENT", "Invalid JSON input"));
      }
    };
    const data = (chunk: Buffer | string) => {
      const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
      bytes += buffer.length;
      if (bytes > limit) {
        finish(new CoreError("RESOURCE_LIMIT", "Agent input budget exceeded"));
        return;
      }
      chunks.push(buffer);
    };
    const end = () => finish(),
      fail = () =>
        finish(new CoreError("TRANSPORT_ERROR", "Agent input unavailable"));
    const abort = () => finish(new CoreError("CANCELLED", "Cancelled"));
    const timer = setTimeout(
      () => finish(new CoreError("TIMEOUT", "Agent input timeout")),
      INPUT_TIMEOUT_MS,
    );
    input.on("data", data);
    input.once("end", end);
    input.once("error", fail);
    signal.addEventListener("abort", abort, { once: true });
    if (signal.aborted) abort();
  });
}
