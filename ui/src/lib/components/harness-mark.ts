/**
 * The letters a harness badge shows, sized for "OMP". A name of up to three characters stays as it is. A longer
 * mixed-case name shows its capitals ("OpenCode" is "OC"). A longer name in capitals only, such as an id upper-cased
 * while the harness list has not arrived, has no word boundaries to read and shows its first two letters ("OP").
 */
export function harnessMark(name: string): string {
  if (name.length <= 3) {
    return name;
  }

  if (name === name.toUpperCase()) {
    return name.slice(0, 2);
  }

  const capitals = name.replace(/[^A-Z]/g, '');

  return capitals.length >= 2 ? capitals.slice(0, 3) : name.slice(0, 2).toUpperCase();
}
