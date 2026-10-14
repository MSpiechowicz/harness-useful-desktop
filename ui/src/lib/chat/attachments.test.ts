import { describe, expect, it } from 'vitest';

import { attachFile, attachmentBytes, base64Of, formatBytes, imageFiles, isLargePaste, pastedTextFile, splitFileNote, toImages, withFileNote, type Attachment } from './attachments';

const file = (name: string, type: string, content = 'hi') => new File([content], name, { type });

describe('image attachments', () => {
  it('keeps only the image types the harnesses accept', () => {
    const files = [file('a.png', 'image/png'), file('b.svg', 'image/svg+xml'), file('c.txt', 'text/plain'), file('d.webp', 'image/webp')];

    expect(imageFiles(files).map((f) => f.name)).toEqual(['a.png', 'd.webp']);
  });

  it('takes the base64 part of a data URL', () => {
    expect(base64Of('data:image/png;base64,aGk=')).toBe('aGk=');
  });

  it('sends only data and type, and counts the base64 bytes', () => {
    const attachment: Attachment = { id: 'i', name: 'a.png', url: 'data:image/png;base64,aGk=', data: 'aGk=', mimeType: 'image/png' };

    expect(toImages([attachment])).toEqual([{ data: 'aGk=', mimeType: 'image/png' }]);
    expect(attachmentBytes([attachment, attachment])).toBe(8);
  });

  it('attaches a small image as it is', async () => {
    const result = await attachFile(file('a.png', 'image/png'), 1024);

    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.attachment).toMatchObject({ name: 'a.png', mimeType: 'image/png', data: btoa('hi') });
    }
  });

  it('refuses other files', async () => {
    expect(await attachFile(file('a.txt', 'text/plain'), 1024)).toMatchObject({ ok: false, reason: 'unsupported' });
  });
});

describe('attached files', () => {
  const files = [
    { name: 'build.log', path: '/home/me/.cache/attachments/1/build.log', bytes: 2048 },
    { name: 'a (1).json', path: '/tmp/a (1).json', bytes: 12 },
  ];

  it('names the files after the text, and reads them back out of a message', () => {
    const message = withFileNote('why?', files);

    expect(splitFileNote(message)).toEqual({
      text: 'why?',
      files: [
        { path: files[0].path, name: 'build.log', size: '2.0 KB' },
        { path: '/tmp/a (1).json', name: 'a (1).json', size: '12 B' },
      ],
    });
    expect(splitFileNote(withFileNote('', files)).text).toBe('');
    expect(withFileNote('hi', [])).toBe('hi');
  });

  it('leaves a message that merely mentions the header alone', () => {
    const text = 'I wrote "Attached files (open them with your file tools):" by hand';

    expect(splitFileNote(text)).toEqual({ text, files: [] });
  });

  it('formats sizes', () => {
    expect([formatBytes(0), formatBytes(1500), formatBytes(50_000), formatBytes(5 * 1024 * 1024)]).toEqual(['0 B', '1.5 KB', '49 KB', '5.0 MB']);
  });

  it('files large pastes by length or line count', () => {
    expect(isLargePaste('short')).toBe(false);
    expect(isLargePaste('x'.repeat(3001))).toBe(true);
    expect(isLargePaste('line\n'.repeat(50))).toBe(true);
    expect(pastedTextFile('hi', new Date('2026-10-14T10:20:30Z')).name).toBe('pasted-text-20261014-102030.txt');
  });
});
