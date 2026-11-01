import { t } from '$lib/i18n/i18n.svelte';
import { IpcFailure } from '$lib/ipc/transport';
import type { RemoteDatabaseProbe } from '$lib/stores/remoteDatabase.svelte';

import { savedUrlFits, takesToken, urlMatchesKind, type RemoteDatabaseConfig } from './remote-database';

/** What the backend has saved of the connection. */
export interface SavedConnection<Kind extends string> {
  enabled: boolean;
  kind: Kind | null;
  /** The saved URL without its password, query and fragment. */
  urlRedacted: string | null;
  hasToken: boolean;
  /** The config file, home-relative for display. */
  configPath: string;
  /** What is wrong with the file, if anything. */
  problem: string | null;
}

/** What a connection test answers. */
export interface ConnectionTested {
  ok: boolean;
  message: string;
}

/**
 * The connection part of a remote database form: the database, the typed URL and token, and what a test or a save shows.
 * The draft starts from what is saved; the secrets start empty, are never read back, and go straight to the backend.
 */
export class RemoteConnectionDraft<Kind extends string> {
  kind: Kind;
  url = $state('');
  token = $state('');
  touched = $state(false);
  busy = $state<'test' | 'save' | null>(null);
  status = $state<{ ok: boolean; text: string } | null>(null);

  constructor(
    readonly config: RemoteDatabaseConfig<Kind>,
    private readonly readSaved: () => SavedConnection<Kind>,
    private readonly probe: (args: RemoteDatabaseProbe<Kind>) => Promise<ConnectionTested>,
    /** A new address or kind starts over: whatever else the form decided about the old database is the owner's to reset. */
    private readonly onDatabaseChanged: () => void,
  ) {
    this.kind = $state(readSaved().kind ?? config.defaultKind);
  }

  /** What the backend has saved; follows it while the form is open. */
  get saved(): SavedConnection<Kind> {
    return this.readSaved();
  }

  get typedUrl(): string {
    return this.url.trim();
  }

  get typedToken(): string {
    return this.token.trim();
  }

  get sameKind(): boolean {
    return this.saved.kind === this.kind;
  }

  get takesToken(): boolean {
    return takesToken(this.config, this.kind);
  }

  get urlInvalid(): boolean {
    return this.typedUrl !== '' && !urlMatchesKind(this.config, this.kind, this.typedUrl);
  }

  /** A typed URL has to fit the database; without one, the saved URL stands in if it is for this database. */
  get savedUrlStands(): boolean {
    return savedUrlFits(this.config, this.kind, this.saved.urlRedacted);
  }

  get urlUsable(): boolean {
    return this.typedUrl !== '' ? !this.urlInvalid : this.savedUrlStands;
  }

  /** The connection differs from what is saved: it is off, another database, or something typed. */
  get connectionChanged(): boolean {
    return !this.saved.enabled || !this.sameKind || this.typedUrl !== '' || this.typedToken !== '';
  }

  /** Whatever a test or a save said is out of date once the form changes. */
  edited(): void {
    this.status = null;
  }

  typeUrl(next: string): void {
    if (next !== this.url) {
      this.onDatabaseChanged();
    }

    this.url = next;
  }

  /** The URL field lost focus: a typed address that does not fit starts to show its problem. */
  blurUrl(): void {
    this.touched ||= this.typedUrl !== '';
  }

  chooseKind(next: Kind): void {
    if (next === this.kind) {
      return;
    }

    this.onDatabaseChanged();
    this.kind = next;
    this.url = '';
    this.token = '';
    this.touched = false;
    this.edited();
  }

  /** The token to send: null keeps the saved one, an empty string clears it (when the database no longer takes one). */
  tokenToSend(): string | null {
    const saved = this.saved;

    if (!this.takesToken) {
      return saved.hasToken && saved.kind !== null && takesToken(this.config, saved.kind) ? '' : null;
    }

    return this.typedToken === '' ? null : this.typedToken;
  }

  /** The secrets leave the form as soon as they are stored. */
  stored(): void {
    this.url = '';
    this.token = '';
    this.touched = false;
    this.status = { ok: true, text: t(this.config.copy.saved) };
  }

  fail(error: unknown): void {
    this.status = { ok: false, text: IpcFailure.from(error).message };
  }

  async test(): Promise<void> {
    this.busy = 'test';
    this.status = null;

    try {
      const result = await this.probe({
        kind: this.kind,
        url: this.typedUrl === '' ? null : this.typedUrl,
        token: this.takesToken && this.typedToken !== '' ? this.typedToken : null,
      });

      this.status = { ok: result.ok, text: result.message };
    } catch (error) {
      this.fail(error);
    } finally {
      this.busy = null;
    }
  }
}
