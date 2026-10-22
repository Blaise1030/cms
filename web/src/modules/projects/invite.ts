import { ApiError } from "@/core/api";
import { errorMessage } from "@orso/utils";

/** GitHub page where an owner adds a teammate to this site, or null without a known repo. */
export function collaboratorsUrl(repo: string | null): string | null {
  if (!repo) return null;
  const parts = repo.split("/");
  if (parts.length !== 2 || !parts[0] || !parts[1]) return null;
  return `https://github.com/${repo}/settings/access`;
}

const ACCESS_MESSAGE = "Ask an owner to add you to this site.";

/** Plain message for opening a site: no access reads as an owner invite, never a git error. */
export function openErrorMessage(e: unknown): string {
  if (e instanceof ApiError && (e.status === 403 || e.status === 404)) return ACCESS_MESSAGE;
  const message = errorMessage(e);
  if (/repository not found|403|forbidden|permission denied|could not read from remote/i.test(message)) {
    return ACCESS_MESSAGE;
  }
  return message;
}
