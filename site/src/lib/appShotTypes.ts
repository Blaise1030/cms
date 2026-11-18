/** One theme's variant of an app screenshot, ready for <img>/<source>. */
export interface AppShotSource {
  src: string;
  srcSet: string;
}

/**
 * A light/dark pair of real Orso app captures (from e2e/scripts/landing-shots.mjs), resolved at
 * build time by `appShot()` in `lib/appShots.ts`. Plain data so it can be passed to React islands.
 */
export interface AppShotData {
  id: string;
  width: number;
  height: number;
  light: AppShotSource;
  dark: AppShotSource;
  /** Short clips under public/app/, when the shot was recorded with video. */
  video?: { light: string; dark: string };
}
