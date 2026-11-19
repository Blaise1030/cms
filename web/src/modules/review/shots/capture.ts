import { cropRect, JPEG_QUALITY, MAX_PNG_BYTES, outputSize, type CssRect } from "./crop";

/** A cropped screenshot, ready to send. */
export interface Shot {
  dataUrl: string;
  mime: "image/png" | "image/jpeg";
}

/** Why there is no screenshot: the browser can't share a tab, or the person said no. */
export class CaptureUnavailable extends Error {
  readonly reason: "unsupported" | "denied";
  constructor(message: string, reason: "unsupported" | "denied") {
    super(message);
    this.reason = reason;
  }
}

/** The tab stream is stopped after this long without a pick. */
export const IDLE_MS = 5 * 60_000;

export function captureSupported(): boolean {
  return typeof navigator !== "undefined" && !!navigator.mediaDevices?.getDisplayMedia;
}

/**
 * Pixels of the review tab itself, for picking areas of cross-origin frames the page can't read.
 * One `getDisplayMedia` stream is kept while picking, so the browser asks once; it stops after
 * 5 idle minutes or when `stop()` is called (leaving the Review page).
 */
export class TabCapture {
  private stream: MediaStream | null = null;
  private video: HTMLVideoElement | null = null;
  private idle: ReturnType<typeof setTimeout> | null = null;

  get active(): boolean {
    return !!this.stream?.getVideoTracks().some((t) => t.readyState === "live");
  }

  /** Asks to share this tab (once while the stream lives). */
  async start(): Promise<void> {
    if (this.active) return this.touch();
    if (!captureSupported()) throw new CaptureUnavailable("This browser can't take screenshots of the page.", "unsupported");
    try {
      this.stream = await navigator.mediaDevices.getDisplayMedia({
        video: { displaySurface: "browser" },
        audio: false,
        // Chromium hints: offer this tab first, and allow choosing it.
        preferCurrentTab: true,
        selfBrowserSurface: "include",
      } as DisplayMediaStreamOptions);
    } catch (e) {
      throw new CaptureUnavailable(
        (e as Error)?.name === "NotAllowedError" ? "Screen sharing was not allowed." : "Couldn't share this tab.",
        "denied",
      );
    }
    // The person can end sharing from the browser's own bar.
    for (const t of this.stream.getVideoTracks()) t.addEventListener("ended", () => this.stop());
    const video = document.createElement("video");
    video.muted = true;
    video.playsInline = true;
    video.srcObject = this.stream;
    await video.play().catch(() => undefined);
    this.video = video;
    this.touch();
  }

  /** The current tab pixels under `rect` (CSS px of the page), cropped and encoded. */
  async grab(rect: CssRect): Promise<Shot> {
    if (!this.active || !this.video) await this.start();
    this.touch();
    const frame = await this.frame();
    const crop = cropRect(rect, { width: frame.width, height: frame.height }, { width: window.innerWidth, height: window.innerHeight });
    const size = outputSize(crop.sw, crop.sh);
    const canvas = document.createElement("canvas");
    canvas.width = size.width;
    canvas.height = size.height;
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new CaptureUnavailable("Couldn't draw the screenshot.", "unsupported");
    ctx.drawImage(frame.source, crop.sx, crop.sy, crop.sw, crop.sh, 0, 0, size.width, size.height);
    return encode(canvas);
  }

  stop() {
    if (this.idle) clearTimeout(this.idle);
    this.idle = null;
    for (const t of this.stream?.getTracks() ?? []) t.stop();
    this.stream = null;
    if (this.video) this.video.srcObject = null;
    this.video = null;
  }

  private touch() {
    if (this.idle) clearTimeout(this.idle);
    this.idle = setTimeout(() => this.stop(), IDLE_MS);
  }

  /** One frame of the stream: ImageCapture where there is one, else the playing video. */
  private async frame(): Promise<{ source: CanvasImageSource; width: number; height: number }> {
    const track = this.stream?.getVideoTracks()[0];
    const Capture = (globalThis as { ImageCapture?: new (t: MediaStreamTrack) => { grabFrame(): Promise<ImageBitmap> } })
      .ImageCapture;
    if (track && Capture) {
      try {
        const bitmap = await new Capture(track).grabFrame();
        return { source: bitmap, width: bitmap.width, height: bitmap.height };
      } catch {
        // Fall back to the video element.
      }
    }
    const video = this.video!;
    if (!video.videoWidth) await new Promise((r) => video.addEventListener("loadeddata", r, { once: true }));
    return { source: video, width: video.videoWidth, height: video.videoHeight };
  }
}

/** PNG, or JPEG at 0.85 when the PNG is over 1 MB. */
export async function encode(canvas: HTMLCanvasElement): Promise<Shot> {
  const png = await toBlob(canvas, "image/png");
  if (png && png.size <= MAX_PNG_BYTES) return { dataUrl: await toDataUrl(png), mime: "image/png" };
  const jpeg = await toBlob(canvas, "image/jpeg", JPEG_QUALITY);
  if (!jpeg) throw new CaptureUnavailable("Couldn't encode the screenshot.", "unsupported");
  return { dataUrl: await toDataUrl(jpeg), mime: "image/jpeg" };
}

const toBlob = (canvas: HTMLCanvasElement, type: string, quality?: number) =>
  new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, type, quality));

const toDataUrl = (blob: Blob) =>
  new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(blob);
  });
