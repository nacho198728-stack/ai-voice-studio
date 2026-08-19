import type {
  CapabilityDto,
  DesktopApi,
  MockPipelineSummaryDto,
  RuntimeState,
  RuntimeStatusDto,
} from "./api";

const stateLabels: Record<RuntimeState, string> = {
  stopped: "Stopped",
  starting: "Starting",
  connected: "Connected",
  stopping: "Stopping",
  crashed: "Crashed",
  error: "Error",
};

interface ViewState {
  status: RuntimeStatusDto;
  pending: string | null;
  error: string | null;
  capability: CapabilityDto | null;
  summary: MockPipelineSummaryDto | null;
}

const initialStatus: RuntimeStatusDto = {
  productName: "AI Voice Studio",
  productVersion: "0.0.0-dev",
  runtimeVersion: "0.0.0",
  state: "stopped",
  generation: 0,
  detail: "Loading Runtime status…",
};

export function mountDesktop(root: HTMLElement, api: DesktopApi): void {
  const state: ViewState = {
    status: initialStatus,
    pending: "Refreshing…",
    error: null,
    capability: null,
    summary: null,
  };

  const action = async <T>(
    pending: string,
    operation: () => Promise<T>,
    accept: (value: T) => void,
  ): Promise<void> => {
    if (state.pending !== null) return;
    state.pending = pending;
    state.error = null;
    render();
    try {
      accept(await operation());
    } catch (error: unknown) {
      state.error = publicError(error);
    } finally {
      state.pending = null;
      render();
    }
  };

  const render = (): void => {
    const connected = state.status.state === "connected";
    const startable = ["stopped", "crashed", "error"].includes(state.status.state);
    const busy = state.pending !== null;
    root.replaceChildren();

    const shell = element("main", "shell");
    const header = element("header", "hero");
    const eyebrow = element("p", "eyebrow", "Local native control plane");
    const title = element("h1", "", state.status.productName);
    const version = element("p", "version", `Development ${state.status.productVersion}`);
    header.append(eyebrow, title, version);

    const statusCard = element("section", "card status-card");
    statusCard.setAttribute("aria-labelledby", "runtime-heading");
    statusCard.setAttribute("aria-live", "polite");
    statusCard.setAttribute("aria-atomic", "true");
    const statusHeading = element("div", "section-heading");
    const heading = element("h2", "", "Runtime");
    heading.id = "runtime-heading";
    const badge = element("span", "status-badge", stateLabels[state.status.state]);
    badge.dataset.state = state.status.state;
    statusHeading.append(heading, badge);
    statusCard.append(
      statusHeading,
      definitionList([
        ["Runtime version", state.status.runtimeVersion],
        ["Generation", String(state.status.generation)],
        ["Status", state.status.detail],
      ]),
    );

    const actions = element("div", "actions");
    actions.append(
      actionButton(
        state.pending === "Starting…" ? "Starting…" : "Start Runtime",
        busy || !startable,
        () => void action("Starting…", api.startRuntime, (value) => (state.status = value)),
        state.pending === "Starting…",
      ),
      actionButton(
        state.pending === "Refreshing…" ? "Refreshing…" : "Refresh Status",
        busy,
        () =>
          void action("Refreshing…", api.getRuntimeStatus, (value) => (state.status = value)),
        state.pending === "Refreshing…",
      ),
      actionButton(
        state.pending === "Querying…" ? "Querying…" : "Query Capabilities",
        busy || !connected,
        () =>
          void action(
            "Querying…",
            api.getRuntimeCapabilities,
            (value) => (state.capability = value),
          ),
        state.pending === "Querying…",
      ),
      actionButton(
        state.pending === "Running…" ? "Running…" : "Run Mock Pipeline",
        busy || !connected,
        () =>
          void action("Running…", api.runMockPipeline, (value) => (state.summary = value)),
        state.pending === "Running…",
      ),
      actionButton(
        state.pending === "Stopping…" ? "Stopping…" : "Stop Runtime",
        busy || !connected,
        () => void action("Stopping…", api.stopRuntime, (value) => (state.status = value)),
        state.pending === "Stopping…",
      ),
    );
    statusCard.append(actions);

    if (state.error !== null) {
      const alert = element("p", "error", state.error);
      alert.setAttribute("role", "alert");
      statusCard.append(alert);
    }

    const results = element("div", "result-grid");
    results.append(capabilityCard(state.capability), summaryCard(state.summary));
    shell.append(header, statusCard, results);
    root.append(shell);
  };

  render();
  void (async () => {
    try {
      state.status = await api.getRuntimeStatus();
    } catch (error: unknown) {
      state.error = publicError(error);
    } finally {
      state.pending = null;
      render();
    }
  })();
}

function capabilityCard(capability: CapabilityDto | null): HTMLElement {
  const card = element("section", "card");
  card.append(element("h2", "", "Capabilities"));
  if (capability === null) {
    card.append(element("p", "empty", "Connect the Runtime, then query its bounded capability summary."));
    return card;
  }
  card.append(
    definitionList([
      ["Platform", `${capability.platform} / ${capability.architecture}`],
      ["Backend", capability.backend],
      ["Engine", capability.engineIdentity ?? "Unavailable"],
      ["Protocol", String(capability.protocolVersion)],
      ["Availability", `${capability.runtimeAvailability} / ${capability.engineAvailability}`],
    ]),
  );
  return card;
}

function summaryCard(summary: MockPipelineSummaryDto | null): HTMLElement {
  const card = element("section", "card");
  card.append(element("h2", "", "Latest Mock Summary"));
  if (summary === null) {
    card.append(element("p", "empty", "No deterministic native summary has been requested."));
    return card;
  }
  card.append(
    definitionList([
      ["Frames", `${summary.inputFrames} input / ${summary.outputFrames} output`],
      ["Checksum", summary.checksum],
      ["Elapsed", `${summary.elapsedMicroseconds} µs`],
      ["Calls", String(summary.processCallCount)],
      ["Errors", String(summary.processErrorCount)],
      ["Stream generation", String(summary.streamGeneration)],
    ]),
  );
  return card;
}

function actionButton(
  label: string,
  disabled: boolean,
  activate: () => void,
  busy: boolean,
): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = label;
  button.disabled = disabled;
  if (busy) button.setAttribute("aria-busy", "true");
  button.addEventListener("click", activate);
  return button;
}

function definitionList(rows: ReadonlyArray<readonly [string, string]>): HTMLDListElement {
  const list = document.createElement("dl");
  for (const [name, value] of rows) {
    list.append(element("dt", "", name), element("dd", "", value));
  }
  return list;
}

function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className = "",
  text = "",
): HTMLElementTagNameMap[K] {
  const value = document.createElement(tag);
  value.className = className;
  value.textContent = text;
  return value;
}

function publicError(error: unknown): string {
  if (typeof error === "object" && error !== null && "message" in error) {
    const message = String(error.message);
    return message.length <= 240 ? message : `${message.slice(0, 239)}…`;
  }
  return "The desktop command failed. Refresh status and retry.";
}
