// Where each session's terminal text goes. The protocol client can deliver output before a
// tile's terminal exists (the font is still loading, say); it waits here until one attaches.

export interface TerminalSink {
  write(text: string): void;
  reset(): void;
}

export class TerminalSinks {
  private readonly entries = new Map<string, { sink?: TerminalSink; pending: string[] }>();

  private entry(session: string) {
    let entry = this.entries.get(session);
    if (!entry) {
      entry = { pending: [] };
      this.entries.set(session, entry);
    }
    return entry;
  }

  write(session: string, text: string): void {
    const entry = this.entry(session);
    if (entry.sink) entry.sink.write(text);
    else entry.pending.push(text);
  }

  reset(session: string): void {
    const entry = this.entry(session);
    entry.pending = [];
    entry.sink?.reset();
  }

  /** Attach a terminal: it gets what is waiting, then everything new. Returns the detach. */
  attach(session: string, sink: TerminalSink): () => void {
    const entry = this.entry(session);
    entry.sink = sink;
    if (entry.pending.length > 0) sink.write(entry.pending.join(""));
    entry.pending = [];
    return () => {
      if (entry.sink === sink) entry.sink = undefined;
    };
  }
}
