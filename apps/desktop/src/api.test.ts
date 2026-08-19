import { beforeEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { desktopApi } from "./api";

describe("typed invoke boundary", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue({});
  });

  it("uses the five explicit input-free command names", async () => {
    await desktopApi.startRuntime();
    await desktopApi.getRuntimeStatus();
    await desktopApi.getRuntimeCapabilities();
    await desktopApi.runMockPipeline();
    await desktopApi.stopRuntime();

    expect(invoke.mock.calls).toEqual([
      ["start_runtime"],
      ["get_runtime_status"],
      ["get_runtime_capabilities"],
      ["run_mock_pipeline"],
      ["stop_runtime"],
    ]);
  });
});
