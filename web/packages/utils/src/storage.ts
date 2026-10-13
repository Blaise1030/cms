// Best-effort localStorage access (unavailable in private mode, SSR and tests).

function store(): Storage | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

export function readPref(key: string): string | null {
  try {
    return store()?.getItem(key) ?? null;
  } catch {
    return null;
  }
}

export function writePref(key: string, value: string | null): void {
  try {
    if (value === null) store()?.removeItem(key);
    else store()?.setItem(key, value);
  } catch {
    // ignore: preferences are optional
  }
}
