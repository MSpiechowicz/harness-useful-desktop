import { t } from '$lib/i18n/i18n.svelte';
import type { AttachedFile } from '$lib/ipc/bindings/AttachedFile';
import type { DroppedImage } from '$lib/ipc/bindings/DroppedImage';
import { IpcFailure, type Transport } from '$lib/ipc/transport';
import type { ToastsStore } from '$lib/stores/toasts.svelte';

import {
  MAX_FILES,
  MAX_IMAGE_BYTES,
  MAX_IMAGES,
  attachFile,
  attachmentBytes,
  fileData,
  fileFromBase64,
  isImageType,
  pastedTextFile,
  type Attachment,
} from './attachments';

/** What the composer holds for the next message. */
export interface Attached {
  images: Attachment[];
  files: AttachedFile[];
}

type Refusal = 'unsupported' | 'tooLarge' | 'unreadable' | 'limit' | 'limitFiles';

/**
 * The images and files waiting in the composer. Images travel inside the message (scaled to fit);
 * every other file is saved on disk (or already is) and the message names its path.
 */
export class ComposerAttachments {
  images = $state.raw<Attachment[]>([]);
  files = $state.raw<AttachedFile[]>([]);

  constructor(
    private readonly transport: Transport,
    private readonly notifications: ToastsStore,
  ) {}

  get count(): number {
    return this.images.length + this.files.length;
  }

  /** Takes everything out of the composer, for sending. */
  take(): Attached {
    const taken = { images: this.images, files: this.files };
    this.images = [];
    this.files = [];

    return taken;
  }

  /** Puts back what `take` took, unless the user attached something new meanwhile. */
  restore(attached: Attached): void {
    if (this.count === 0) {
      this.images = attached.images;
      this.files = attached.files;
    }
  }

  removeImage(id: string): void {
    this.images = this.images.filter((image) => image.id !== id);
  }

  removeFile(path: string): void {
    this.files = this.files.filter((file) => file.path !== path);
  }

  /** Adds files the webview holds: images join the message, anything else is saved for the agent to open. */
  async add(files: readonly File[]): Promise<void> {
    for (const file of files) {
      const refusal = await this.addOne(file);
      if (refusal !== null) {
        this.refuse(file.name || 'file', refusal);
      }
    }
  }

  /** Adds pasted text as a file; large pastes would otherwise bury the composer. */
  async addText(text: string): Promise<void> {
    await this.add([pastedTextFile(text)]);
  }

  /** Adds what a native file drop held: images the shell read, files it only named. */
  async addDropped(images: readonly DroppedImage[], files: readonly AttachedFile[], skipped: readonly string[]): Promise<void> {
    for (const name of skipped) {
      this.refuse(name, 'unsupported');
    }

    for (const file of files) {
      if (this.files.some((known) => known.path === file.path)) {
        continue;
      }

      if (this.files.length >= MAX_FILES) {
        this.refuse(file.name, 'limitFiles');
      } else {
        this.files = [...this.files, file];
      }
    }

    await this.add(images.map((image) => fileFromBase64(image.name, image.mimeType, image.data)));
  }

  private refuse(name: string, reason: Refusal): void {
    this.notifications.push({ title: t('chat.attach.failed', { name }), description: t(`chat.attach.${reason}`, { max: reason === 'limitFiles' ? MAX_FILES : MAX_IMAGES }), tone: 'warning' });
  }

  private async addOne(file: File): Promise<Refusal | null> {
    return isImageType(file.type) ? this.addImage(file) : this.addFile(file);
  }

  private async addImage(file: File): Promise<Refusal | null> {
    if (this.images.length >= MAX_IMAGES) {
      return 'limit';
    }

    const result = await attachFile(file, MAX_IMAGE_BYTES - attachmentBytes(this.images));
    if (!result.ok) {
      return result.reason;
    }

    // Other images may have been added while this one was read.
    if (this.images.length >= MAX_IMAGES) {
      return 'limit';
    }

    this.images = [...this.images, result.attachment];

    return null;
  }

  private async addFile(file: File): Promise<Refusal | null> {
    if (this.files.length >= MAX_FILES) {
      return 'limitFiles';
    }

    try {
      const saved = await this.transport.invoke('attachment_save', { name: file.name || 'attachment', data: await fileData(file) });
      this.files = [...this.files, saved];

      return null;
    } catch (error) {
      this.notifications.push({ title: t('chat.attach.failed', { name: file.name || 'file' }), description: IpcFailure.from(error).message, tone: 'warning' });

      return null;
    }
  }
}
