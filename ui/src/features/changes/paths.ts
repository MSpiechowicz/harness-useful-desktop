/** A path in two pieces: the folder with its trailing slash, and the file name. */
export function splitPath(path: string): { dir: string; name: string } {
  const cut = path.lastIndexOf('/');

  return cut === -1 ? { dir: '', name: path } : { dir: path.slice(0, cut + 1), name: path.slice(cut + 1) };
}
