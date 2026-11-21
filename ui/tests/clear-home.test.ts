import { describe, expect, it } from "vitest";
import { ClearHome } from "../src/lib/clear-home";
import { TerminalSinks } from "../src/lib/sinks";

const CLEAR = "\x1b[2J";
const HOME = "\x1b[H";

describe("ClearHome", () => {
  it("homes the cursor after a clear", () => {
    const c = new ClearHome();
    expect(c.feed(`a\r\n\x1b[0m${CLEAR}\x1b[0mform`)).toBe(
      `a\r\n\x1b[0m${CLEAR}${HOME}\x1b[0mform`,
    );
  });

  it("finds a clear split across chunks", () => {
    for (let at = 1; at < CLEAR.length; at++) {
      const c = new ClearHome();
      const first = c.feed(`ab${CLEAR.slice(0, at)}`);
      expect(first).toBe("ab");
      expect(first + c.feed(`${CLEAR.slice(at)}cd`)).toBe(
        `ab${CLEAR}${HOME}cd`,
      );
    }
  });

  it("leaves other sequences alone", () => {
    const c = new ClearHome();
    const text = "\x1b[2K\x1b[J\x1b[1;37mhi\x1b[20;5f";
    expect(c.feed(text)).toBe(text);
    // A held prefix that turns out not to be a clear goes out with the next chunk.
    expect(c.feed("x\x1b[2") + c.feed("Kz")).toBe("x\x1b[2Kz");
  });

  it("drops a held tail on reset", () => {
    const c = new ClearHome();
    c.feed("\x1b[");
    c.reset();
    expect(c.feed("2J")).toBe("2J");
  });
});

describe("TerminalSinks", () => {
  it("homes after a clear, before and after a terminal attaches", () => {
    const sinks = new TerminalSinks();
    sinks.write("s", "x\x1b[");
    sinks.write("s", "2Jy");
    let got = "";
    sinks.attach("s", { write: (t) => (got += t), reset: () => {} });
    sinks.write("s", `${CLEAR}z`);
    expect(got).toBe(`x${CLEAR}${HOME}y${CLEAR}${HOME}z`);
  });
});
