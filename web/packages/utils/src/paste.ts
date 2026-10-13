const END = "\x1b[201~";

/** Wrap text so the CLI treats it as one paste; an embedded end marker is stripped. */
export function bracketedPaste(text: string): string {
  return "\x1b[200~" + text.split(END).join("") + END;
}
