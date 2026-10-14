import type { AttachedFile } from '$lib/ipc/bindings/AttachedFile';
import type { ImageAttachment } from '$lib/ipc/bindings/ImageAttachment';
import { uid } from '$lib/utils/ids';

/** Mirrors `MAX_ACTION_IMAGES` and `MAX_ACTION_IMAGE_BYTES` in the business crate. */
export const MAX_IMAGES = 8;
export const MAX_IMAGE_BYTES = 3 * 1024 * 1024;

export const IMAGE_TYPES = ['image/png', 'image/jpeg', 'image/gif', 'image/webp'];

/** Longest edge and JPEG quality tried in turn until an image fits the remaining budget. */
const SHRINK_STEPS: readonly (readonly [number, number])[] = [
  [2048, 0.85],
  [1600, 0.8],
  [1280, 0.7],
  [1024, 0.6],
  [768, 0.5],
];

/** An image waiting in the composer; `url` is a data URL for the thumbnail. */
export interface Attachment extends ImageAttachment {
  id: string;
  name: string;
  url: string;
}

export type AttachFailure = 'unsupported' | 'tooLarge' | 'unreadable' | 'limit';

export type AttachResult = { ok: true; attachment: Attachment } | { ok: false; reason: AttachFailure; name: string };

export function isImageType(mimeType: string): boolean {
  return IMAGE_TYPES.includes(mimeType);
}

/** The wire form of attachments: the thumbnail and name stay in the webview. */
export function toImages(attachments: readonly Attachment[]): ImageAttachment[] {
  return attachments.map(({ data, mimeType }) => ({ data, mimeType }));
}

export function attachmentBytes(attachments: readonly Pick<ImageAttachment, 'data'>[]): number {
  return attachments.reduce((sum, attachment) => sum + attachment.data.length, 0);
}

/** The base64 part of a `data:` URL. */
export function base64Of(dataUrl: string): string {
  return dataUrl.slice(dataUrl.indexOf(',') + 1);
}

/** The image files among `files` (a drop or paste may carry anything). */
export function imageFiles(files: Iterable<File>): File[] {
  return Array.from(files).filter((file) => isImageType(file.type));
}

/** A `File` for a dropped image the shell read (base64 bytes). */
export function fileFromBase64(name: string, mimeType: string, data: string): File {
  const bytes = Uint8Array.from(atob(data), (char) => char.charCodeAt(0));

  return new File([bytes], name, { type: mimeType });
}

function readAsDataUrl(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(blob);
  });
}

function dataUrlOf(canvas: HTMLCanvasElement, quality: number): string {
  return canvas.toDataURL('image/jpeg', quality);
}

/** Re-encodes `bitmap` as JPEG until it fits `budget` base64 bytes; null when even the smallest step does not. */
function shrink(bitmap: ImageBitmap, budget: number): string | null {
  for (const [edge, quality] of SHRINK_STEPS) {
    const scale = Math.min(1, edge / Math.max(bitmap.width, bitmap.height));
    const canvas = document.createElement('canvas');
    canvas.width = Math.max(1, Math.round(bitmap.width * scale));
    canvas.height = Math.max(1, Math.round(bitmap.height * scale));

    const context = canvas.getContext('2d');
    if (!context) {
      return null;
    }

    // JPEG has no alpha: paint transparency white rather than black.
    context.fillStyle = '#fff';
    context.fillRect(0, 0, canvas.width, canvas.height);
    context.drawImage(bitmap, 0, 0, canvas.width, canvas.height);

    const url = dataUrlOf(canvas, quality);
    if (base64Of(url).length <= budget) {
      return url;
    }
  }

  return null;
}

/**
 * Turns an image file into an attachment that fits `budget` (base64 bytes left for the message).
 * A file that already fits is sent as it is; a larger one is scaled down and re-encoded as JPEG.
 */
export async function attachFile(file: File, budget: number): Promise<AttachResult> {
  const name = file.name || 'image';

  if (!isImageType(file.type)) {
    return { ok: false, reason: 'unsupported', name };
  }

  try {
    const original = await readAsDataUrl(file);
    if (base64Of(original).length <= budget) {
      return { ok: true, attachment: { id: uid('img'), name, mimeType: file.type, data: base64Of(original), url: original } };
    }

    const bitmap = await createImageBitmap(file);
    const shrunk = shrink(bitmap, budget);
    bitmap.close();

    if (shrunk === null) {
      return { ok: false, reason: 'tooLarge', name };
    }

    return { ok: true, attachment: { id: uid('img'), name, mimeType: 'image/jpeg', data: base64Of(shrunk), url: shrunk } };
  } catch {
    return { ok: false, reason: 'unreadable', name };
  }
}

/** Most files that can go with one message (images count separately). */
export const MAX_FILES = 10;

/** Pasted text longer than this becomes a file instead of filling the composer. */
const PASTE_FILE_CHARS = 3000;
const PASTE_FILE_LINES = 40;

export function isLargePaste(text: string): boolean {
  return text.length > PASTE_FILE_CHARS || text.split('\n').length > PASTE_FILE_LINES;
}

/** A text file for a large paste, named after the moment it was pasted. */
export function pastedTextFile(text: string, now: Date = new Date()): File {
  const stamp = now.toISOString().slice(0, 19).replace(/[-:]/g, '').replace('T', '-');

  return new File([text], `pasted-text-${stamp}.txt`, { type: 'text/plain' });
}

/** The base64 content of a file. */
export async function fileData(file: Blob): Promise<string> {
  return base64Of(await readAsDataUrl(file));
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }

  return bytes < 1024 * 1024 ? `${(bytes / 1024).toFixed(bytes < 10 * 1024 ? 1 : 0)} KB` : `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Opens the list of attached files the message carries, so the agent knows to read them. */
export const FILE_NOTE_HEADER = 'Attached files (open them with your file tools):';

/** `text` followed by the attached files, one path per line; the agent reads them from disk. */
export function withFileNote(text: string, files: readonly AttachedFile[]): string {
  if (files.length === 0) {
    return text;
  }

  const lines = files.map((file) => `- ${file.path} (${formatBytes(file.bytes)})`);

  return `${text === '' ? '' : `${text}\n\n`}${FILE_NOTE_HEADER}\n${lines.join('\n')}`;
}

export interface FileRef {
  path: string;
  name: string;
  size: string;
}

/** Splits a message into its text and the attached files named by a trailing {@link withFileNote} block. */
export function splitFileNote(message: string): { text: string; files: FileRef[] } {
  const start = message.lastIndexOf(FILE_NOTE_HEADER);
  const atStart = start === 0 || message.slice(start - 2, start) === '\n\n';
  if (start === -1 || !atStart) {
    return { text: message, files: [] };
  }

  const lines = message.slice(start + FILE_NOTE_HEADER.length).split('\n').slice(1);
  const files = lines.map((line) => {
    const open = line.lastIndexOf(' (');
    if (!line.startsWith('- ') || open === -1 || !line.endsWith(')')) {
      return null;
    }

    const path = line.slice(2, open);

    return { path, name: path.split(/[\\/]/).pop() ?? path, size: line.slice(open + 2, -1) };
  });

  if (files.length === 0 || files.some((file) => file === null)) {
    return { text: message, files: [] };
  }

  return { text: message.slice(0, Math.max(0, start - 2)), files: files as FileRef[] };
}
