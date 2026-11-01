/** What the form sends to try a connection: the token is null to keep the saved one. */
export interface RemoteDatabaseProbe<Kind extends string> {
  kind: Kind;
  url: string | null;
  token: string | null;
}

/** The commands of one Harness family member that read, write and test its team database. */
export interface RemoteDatabaseCommands<Remote, Save, Probe, Tested> {
  get: () => Promise<Remote>;
  set: (args: Save) => Promise<Remote>;
  test: (args: Probe) => Promise<Tested>;
}

/**
 * The team-database setting of a Harness family member. It only ever holds what the backend reports (the URL redacted, never the
 * token); what the user types is kept by the form that owns it and goes straight to the backend, so no secret lands in
 * a store, in storage or in a log. Failures are thrown for the form to show beside its fields.
 */
export class RemoteDatabaseStore<Remote, Save, Probe, Tested> {
  value = $state<Remote | null>(null);

  /** Set when the last read failed, so the form can say so instead of showing a stale or empty panel. */
  loadFailed = $state(false);

  constructor(private readonly commands: RemoteDatabaseCommands<Remote, Save, Probe, Tested>) {}

  async load(): Promise<void> {
    try {
      this.value = await this.commands.get();
      this.loadFailed = false;
    } catch {
      this.loadFailed = true;
    }
  }

  async save(args: Save): Promise<Remote> {
    const saved = await this.commands.set(args);
    this.value = saved;
    this.loadFailed = false;

    return saved;
  }

  test(args: Probe): Promise<Tested> {
    return this.commands.test(args);
  }
}
