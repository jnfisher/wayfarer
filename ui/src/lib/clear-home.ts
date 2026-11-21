// ANSI.SYS clears the screen and homes the cursor on `ESC[2J`; xterm.js, like every VT
// terminal, leaves the cursor where it was. BBS games are written for the first: a server
// that clears, draws a form and then places its fields by absolute row would otherwise draw
// the form from the row the cursor was on, and the fields would land above it. So each
// `ESC[2J` gets an `ESC[H` after it. The text arrives in chunks, so a chunk ending part-way
// into the sequence keeps that part back for the next one.

const CLEAR = "\x1b[2J";
const HOME = "\x1b[H";

export class ClearHome {
  private held = "";

  /** The text to write for `text`, with a held-back tail from the last chunk in front. */
  feed(text: string): string {
    let all = this.held + text;
    this.held = "";
    for (let n = Math.min(CLEAR.length - 1, all.length); n > 0; n--) {
      if (CLEAR.startsWith(all.slice(-n))) {
        this.held = all.slice(-n);
        all = all.slice(0, -n);
        break;
      }
    }
    return all.replaceAll(CLEAR, CLEAR + HOME);
  }

  /** Forget a held-back tail (the terminal was reset). */
  reset(): void {
    this.held = "";
  }
}
