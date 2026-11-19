import { describe, expect, it } from "vitest";
import { MAX_IMAGE_BYTES, pickImages } from "./attachments";

const file = (name: string, type: string, size = 10) => {
  const f = new File([new Uint8Array(1)], name, { type });
  Object.defineProperty(f, "size", { value: size });
  return f;
};

describe("pickImages", () => {
  it("takes PNG, JPEG, GIF and WebP images up to 5 MB, ten in all", () => {
    const ok = ["a.png", "b.jpg", "c.gif", "d.webp"].map((n, i) =>
      file(n, ["image/png", "image/jpeg", "image/gif", "image/webp"][i]!),
    );
    expect(pickImages(ok, 0)).toEqual({ accepted: ok, refused: null });
  });

  it("leaves out other types, big files and the eleventh, saying why", () => {
    const { accepted, refused } = pickImages(
      [file("x.svg", "image/svg+xml"), file("big.png", "image/png", MAX_IMAGE_BYTES + 1), file("ok.png", "image/png"), file("more.png", "image/png")],
      9,
    );
    expect(accepted.map((f) => f.name)).toEqual(["ok.png"]);
    expect(refused).toContain("Only PNG, JPEG, GIF and WebP");
    expect(refused).toContain("big.png is larger than 5 MB");
    expect(refused).toContain("at most 10 images");
  });
});
