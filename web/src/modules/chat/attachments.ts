export type SentAttachment = { name: string; kind: string; image: boolean };

const IMAGE_EXTS = ["png", "jpg", "gif", "webp"];
/** A saved path the daemon's attach endpoint returned, alone on its line (spaces escaped). */
const PATH_LINE = /^(?:\\ |\S)*\/attachments\/([0-9a-f]{16}\.(png|jpg|gif|webp|pdf|txt|md|csv|json))$/;

const standIn = (n: number) => `Please review the attached ${n > 1 ? "files" : "file"}:`;

/** The text sent to the agent: what the user typed, then each saved path on its own line. */
export function attachmentMessage(text: string, paths: string[]): string {
  if (!paths.length) return text;
  const escaped = paths.map((p) => p.replace(/ /g, "\\ "));
  return [text.trim() || standIn(paths.length), ...escaped].join("\n");
}

/** Undo [attachmentMessage] for display: the typed text and the files that followed it. */
export function splitAttachments(text: string): { text: string; files: SentAttachment[] } {
  const lines = text.split("\n");
  const files: SentAttachment[] = [];
  while (lines.length) {
    const m = PATH_LINE.exec(lines.at(-1)!);
    if (!m) break;
    lines.pop();
    files.unshift({ name: m[1]!, kind: m[2]!.toUpperCase(), image: IMAGE_EXTS.includes(m[2]!) });
  }
  if (!files.length) return { text, files };
  const rest = lines.join("\n");
  return { text: rest === standIn(files.length) ? "" : rest, files };
}
