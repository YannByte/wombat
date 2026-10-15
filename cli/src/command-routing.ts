/** Find a command without loading command implementations or presentation code. */
export function firstCommandIndex(argv: readonly string[]): number {
  let index = 0;
  while (argv[index] === "--lang" || argv[index]?.startsWith("--lang=")) {
    index += argv[index] === "--lang" ? 2 : 1;
  }
  return index;
}
