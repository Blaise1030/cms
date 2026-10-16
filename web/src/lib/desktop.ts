/**
 * The macOS desktop shell marks `<html data-desktop="macos">` before the app loads. Its window
 * has no title bar, so the app reserves room for the traffic lights and drags the window from
 * its top bars. Browsers and the PWA never see the marker.
 */
export function isMacDesktop(): boolean {
  return document.documentElement.dataset.desktop === "macos";
}
