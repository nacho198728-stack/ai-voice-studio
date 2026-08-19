import { invoke } from "@tauri-apps/api/core";

export type RuntimeState =
  | "stopped"
  | "starting"
  | "connected"
  | "stopping"
  | "crashed"
  | "error";

export interface RuntimeStatusDto {
  productName: string;
  productVersion: string;
  runtimeVersion: string;
  state: RuntimeState;
  generation: number;
  detail: string;
}

export interface CapabilityDto {
  schemaVersion: number;
  platform: string;
  architecture: string;
  runtimeVersion: string;
  protocolVersion: number;
  backend: string;
  runtimeAvailability: string;
  engineIdentity: string | null;
  engineAvailability: string;
  generation: number;
}

export interface MockPipelineSummaryDto {
  inputFrames: number;
  outputFrames: number;
  checksum: string;
  elapsedMicroseconds: number;
  processCallCount: number;
  processErrorCount: number;
  streamGeneration: number;
}

export interface DesktopApi {
  startRuntime(): Promise<RuntimeStatusDto>;
  getRuntimeStatus(): Promise<RuntimeStatusDto>;
  getRuntimeCapabilities(): Promise<CapabilityDto>;
  runMockPipeline(): Promise<MockPipelineSummaryDto>;
  stopRuntime(): Promise<RuntimeStatusDto>;
}

export const desktopApi: DesktopApi = {
  startRuntime: () => invoke<RuntimeStatusDto>("start_runtime"),
  getRuntimeStatus: () => invoke<RuntimeStatusDto>("get_runtime_status"),
  getRuntimeCapabilities: () => invoke<CapabilityDto>("get_runtime_capabilities"),
  runMockPipeline: () => invoke<MockPipelineSummaryDto>("run_mock_pipeline"),
  stopRuntime: () => invoke<RuntimeStatusDto>("stop_runtime"),
};
