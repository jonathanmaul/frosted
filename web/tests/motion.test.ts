import { afterEach, describe, expect, it, vi } from "vitest";
import { canRender3D } from "@/lib/motion";

afterEach(() => vi.restoreAllMocks());
function device({ reduced = false, cores = 8, memory = 8, webgl = true } = {}) {
  vi.spyOn(window, "matchMedia").mockReturnValue({ matches: reduced } as MediaQueryList);
  vi.spyOn(navigator, "hardwareConcurrency", "get").mockReturnValue(cores);
  Object.defineProperty(navigator, "deviceMemory", { configurable: true, value: memory });
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(webgl ? {} as WebGL2RenderingContext : null);
}

describe("3D capability gate", () => {
  it("uses static fallback for reduced motion, weak hardware, and missing WebGL", () => {
    // jsdom does not implement matchMedia.
    Object.defineProperty(window, "matchMedia", { configurable: true, writable: true, value: () => ({ matches: false }) });
    device({ reduced: true }); expect(canRender3D()).toBe(false);
    device({ cores: 4 }); expect(canRender3D()).toBe(false);
    device({ memory: 4 }); expect(canRender3D()).toBe(false);
    device({ webgl: false }); expect(canRender3D()).toBe(false);
    device(); expect(canRender3D()).toBe(true);
  });
});
