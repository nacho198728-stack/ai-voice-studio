import { afterEach, describe, expect, it, vi } from "vitest";

import type { DesktopApi, RuntimeStatusDto } from "./api";
import { mountDesktop } from "./app";

const stopped: RuntimeStatusDto = {
  productName: "AI Voice Studio",
  productVersion: "0.0.0-dev",
  runtimeVersion: "0.0.0",
  state: "stopped",
  generation: 0,
  detail: "Runtime is stopped.",
};

function api(overrides: Partial<DesktopApi> = {}): DesktopApi {
  return {
    startRuntime: vi.fn(async () => ({ ...stopped, state: "connected" as const, generation: 1 })),
    getRuntimeStatus: vi.fn(async () => stopped),
    getRuntimeCapabilities: vi.fn(async () => ({
      schemaVersion: 1,
      platform: "macos",
      architecture: "arm64",
      runtimeVersion: "0.0.0",
      protocolVersion: 1,
      backend: "mock",
      runtimeAvailability: "available",
      engineIdentity: "aivs-mock-v1",
      engineAvailability: "available",
      generation: 1,
    })),
    runMockPipeline: vi.fn(async () => ({
      inputFrames: 128,
      outputFrames: 128,
      checksum: "3ecd5190f6f4f725",
      elapsedMicroseconds: 17,
      processCallCount: 1,
      processErrorCount: 0,
      streamGeneration: 1,
    })),
    stopRuntime: vi.fn(async () => stopped),
    ...overrides,
  };
}

function button(name: string): HTMLButtonElement {
  const element = Array.from(document.querySelectorAll("button")).find(
    (candidate) => candidate.textContent?.trim() === name,
  );
  if (!(element instanceof HTMLButtonElement)) throw new Error(`missing button ${name}`);
  return element;
}

async function flush(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

afterEach(() => {
  document.body.replaceChildren();
});

describe("desktop shell", () => {
  it("renders the bounded product and lifecycle surface from the initial status", async () => {
    mountDesktop(document.body, api());
    await flush();

    expect(document.querySelector("h1")?.textContent).toBe("AI Voice Studio");
    expect(document.body.textContent).toContain("Development 0.0.0-dev");
    expect(document.querySelector('[data-state="stopped"]')?.textContent).toContain("Stopped");
    expect(button("Start Runtime").disabled).toBe(false);
    expect(button("Stop Runtime").disabled).toBe(true);
  });

  it("excludes conflicting actions while a command is pending and recovers after success", async () => {
    let resolveStart: ((value: RuntimeStatusDto) => void) | undefined;
    const start = new Promise<RuntimeStatusDto>((resolve) => {
      resolveStart = resolve;
    });
    mountDesktop(document.body, api({ startRuntime: vi.fn(() => start) }));
    await flush();

    const startButton = button("Start Runtime");
    const refreshButton = button("Refresh Status");
    startButton.focus();
    startButton.click();
    expect(Array.from(document.querySelectorAll("button")).every((item) => item.disabled)).toBe(true);
    expect(button("Starting…").getAttribute("aria-busy")).toBe("true");
    expect(document.activeElement).toBe(document.querySelector(".status-card"));

    resolveStart?.({ ...stopped, state: "connected", generation: 1 });
    await flush();
    expect(button("Stop Runtime").disabled).toBe(false);
    expect(document.querySelector('[data-state="connected"]')).not.toBeNull();
    expect(document.activeElement).toBe(refreshButton);
    expect(startButton.textContent).toBe("Start Runtime");
  });

  it("keeps keyboard focus on refresh across pending and status updates", async () => {
    let resolveRefresh: ((value: RuntimeStatusDto) => void) | undefined;
    const refresh = new Promise<RuntimeStatusDto>((resolve) => {
      resolveRefresh = resolve;
    });
    const getRuntimeStatus = vi
      .fn<() => Promise<RuntimeStatusDto>>()
      .mockResolvedValueOnce(stopped)
      .mockImplementationOnce(() => refresh);
    mountDesktop(document.body, api({ getRuntimeStatus }));
    await flush();

    const refreshButton = button("Refresh Status");
    refreshButton.focus();
    refreshButton.click();
    expect(document.activeElement).toBe(document.querySelector(".status-card"));
    expect(refreshButton.textContent).toBe("Refreshing…");
    expect(refreshButton.disabled).toBe(true);

    resolveRefresh?.({ ...stopped, state: "connected", generation: 1 });
    await flush();
    expect(document.activeElement).toBe(refreshButton);
    expect(refreshButton.textContent).toBe("Refresh Status");
    expect(refreshButton.disabled).toBe(false);
    expect(document.querySelector('[data-state="connected"]')).not.toBeNull();
  });

  it("shows a bounded actionable error and restores controls after command failure", async () => {
    mountDesktop(
      document.body,
      api({
        startRuntime: vi.fn(async () => {
          throw { code: "runtime_unavailable", message: "Build native artifacts, then retry." };
        }),
      }),
    );
    await flush();

    const startButton = button("Start Runtime");
    startButton.focus();
    startButton.click();
    await flush();
    expect(document.querySelector('[role="alert"]')?.textContent).toContain(
      "Build native artifacts, then retry.",
    );
    expect(button("Start Runtime").disabled).toBe(false);
    expect(document.activeElement).toBe(startButton);
  });

  it("keeps lifecycle updates accessible and bounds hostile error text", async () => {
    mountDesktop(
      document.body,
      api({
        startRuntime: vi.fn(async () => {
          throw { message: "💥".repeat(500) };
        }),
      }),
    );
    await flush();

    expect(document.querySelector(".status-card")?.getAttribute("aria-live")).toBe("polite");
    expect(
      Array.from(document.querySelectorAll("button")).every((item) => item.type === "button"),
    ).toBe(true);
    button("Start Runtime").click();
    await flush();
    expect(document.querySelector('[role="alert"]')?.textContent?.length).toBeLessThanOrEqual(240);
  });

  it("renders capabilities and a summary without a PCM or sample-array contract", async () => {
    mountDesktop(document.body, api());
    await flush();
    button("Start Runtime").click();
    await flush();

    button("Query Capabilities").click();
    await flush();
    expect(document.body.textContent).toContain("aivs-mock-v1");
    expect(document.body.textContent).toContain("macos / arm64");

    button("Run Mock Pipeline").click();
    await flush();
    expect(document.body.textContent).toContain("3ecd5190f6f4f725");
    expect(document.body.textContent).toContain("128 input / 128 output");
    expect(document.body.innerHTML.toLowerCase()).not.toContain("pcm");
    expect(document.body.innerHTML.toLowerCase()).not.toContain("samples");
  });

  it("refreshes all six lifecycle states and stops cleanly", async () => {
    const statuses: RuntimeStatusDto[] = [
      { ...stopped, state: "starting" },
      { ...stopped, state: "connected", generation: 1 },
      { ...stopped, state: "stopping", generation: 1 },
      { ...stopped, state: "crashed", generation: 1 },
      { ...stopped, state: "error", generation: 1 },
      stopped,
    ];
    const getRuntimeStatus = vi.fn(async () => statuses.shift() ?? stopped);
    mountDesktop(document.body, api({ getRuntimeStatus }));

    for (const expected of ["starting", "connected", "stopping", "crashed", "error", "stopped"]) {
      await flush();
      expect(document.querySelector(`[data-state="${expected}"]`)).not.toBeNull();
      if (expected !== "stopped") button("Refresh Status").click();
    }
  });
});
