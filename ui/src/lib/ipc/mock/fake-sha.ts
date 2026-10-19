/** A stand-in for the SHA-256 of a file: 64 hex characters that change with every byte of what the file holds. */
export function fakeSha(value: unknown): string {
  const text = JSON.stringify(value);
  let hash = 0x811c9dc5;

  return Array.from({ length: 8 }, (_, round) => {
    for (let index = 0; index < text.length; index += 1) {
      hash = Math.imul(hash ^ (text.charCodeAt(index) + round), 0x01000193) >>> 0;
    }

    return hash.toString(16).padStart(8, '0');
  }).join('');
}
