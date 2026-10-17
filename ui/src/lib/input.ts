// A tile's command history for its input line (Up and Down, like a shell).

export class InputHistory {
  private readonly lines: string[] = [];
  /** Where Up/Down are: `lines.length` means the line being typed. */
  private pos = 0;
  /** What was being typed before the first Up. */
  private draft = "";

  constructor(private readonly max = 500) {}

  /** A line was sent: remember it (not blank lines, not the same line twice running). */
  push(line: string): void {
    if (line.trim() !== "" && this.lines.at(-1) !== line) {
      this.lines.push(line);
      if (this.lines.length > this.max) this.lines.shift();
    }
    this.pos = this.lines.length;
    this.draft = "";
  }

  /** The line before; `current` is what the input holds now. */
  up(current: string): string {
    if (this.pos === this.lines.length) this.draft = current;
    if (this.pos > 0) this.pos -= 1;
    return this.lines[this.pos] ?? current;
  }

  /** The line after, back to the draft at the end. */
  down(): string {
    if (this.pos < this.lines.length) this.pos += 1;
    return this.pos === this.lines.length ? this.draft : this.lines[this.pos];
  }
}
