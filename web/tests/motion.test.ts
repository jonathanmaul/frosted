import { afterEach, describe, expect, it, vi } from "vitest";
import { canRender3D } from "@/lib/motion";

afterEach(() => vi.restoreAllMocks());
function device({ reduced = false, cores = 8, memory = 8, webgl = true, renderer = "Apple M2" } = {}) {
  vi.spyOn(window, "matchMedia").mockReturnValue({ matches: reduced } as MediaQueryList);
  vi.spyOn(navigator, "hardwareConcurrency", "get").mockReturnValue(cores);
  Object.defineProperty(navigator, "deviceMemory", { configurable: true, value: memory });
  const gl = { getExtension: () => ({ UNMASKED_RENDERER_WEBGL: 1 }), getParameter: () => renderer } as unknown as WebGL2RenderingContext;
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(webgl ? gl : null);
}

describe("3D capability gate", () => {
  it("uses static fallback for reduced motion, no GPU, very low memory, and missing WebGL", () => {
    // jsdom does not implement matchMedia.
    Object.defineProperty(window, "matchMedia", { configurable: true, writable: true, value: () => ({ matches: false }) });
    device({ reduced: true }); expect(canRender3D()).toBe(false);
    device({ renderer: "Google SwiftShader" }); expect(canRender3D()).toBe(false);
    device({ memory: 1 }); expect(canRender3D()).toBe(false);
    device({ webgl: false }); expect(canRender3D()).toBe(false);
    device(); expect(canRender3D()).toBe(true);
  });
  it("ignores the core count, which privacy browsers randomize (Brave)", () => {
    Object.defineProperty(window, "matchMedia", { configurable: true, writable: true, value: () => ({ matches: false }) });
    device({ cores: 2 }); expect(canRender3D()).toBe(true);
  });
});
