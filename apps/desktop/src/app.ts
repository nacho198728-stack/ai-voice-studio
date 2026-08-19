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
    const focusedAction =
      document.activeElement instanceof HTMLButtonElement && root.contains(document.activeElement)
        ? document.activeElement
        : null;
    state.pending = pending;
    state.error = null;
    render();
    if (focusedAction !== null) statusCard.focus({ preventScroll: true });
    try {
      accept(await operation());
    } catch (error: unknown) {
      state.error = publicError(error);
    } finally {
      state.pending = null;
      render();
      const focusTarget =
        focusedAction !== null && !focusedAction.disabled
          ? focusedAction
          : [startButton, refreshButton, capabilityButton, summaryButton, stopButton].find(
              (button) => !button.disabled,
            );
      focusTarget?.focus({ preventScroll: true });
    }
  };

  const shell = element("main", "shell");
  const header = element("header", "hero");
  const eyebrow = element("p", "eyebrow", "Local native control plane");
  const title = element("h1");
  const version = element("p", "version");
  header.append(eyebrow, title, version);

  const statusCard = element("section", "card status-card");
  statusCard.tabIndex = -1;
  statusCard.setAttribute("aria-labelledby", "runtime-heading");
  statusCard.setAttribute("aria-live", "polite");
  statusCard.setAttribute("aria-atomic", "true");
  const statusHeading = element("div", "section-heading");
  const heading = element("h2", "", "Runtime");
  heading.id = "runtime-heading";
  const badge = element("span", "status-badge");
  statusHeading.append(heading, badge);

  const statusList = document.createElement("dl");
  const runtimeVersion = appendDefinition(statusList, "Runtime version");
  const generation = appendDefinition(statusList, "Generation");
  const statusDetail = appendDefinition(statusList, "Status");

  const actions = element("div", "actions");
  const startButton = actionButton(() =>
    void action("Starting…", api.startRuntime, (value) => (state.status = value)),
  );
  const refreshButton = actionButton(() =>
    void action("Refreshing…", api.getRuntimeStatus, (value) => (state.status = value)),
  );
  const capabilityButton = actionButton(() =>
    void action(
      "Querying…",
      api.getRuntimeCapabilities,
      (value) => (state.capability = value),
    ),
  );
  const summaryButton = actionButton(() =>
    void action("Running…", api.runMockPipeline, (value) => (state.summary = value)),
  );
  const stopButton = actionButton(() =>
    void action("Stopping…", api.stopRuntime, (value) => (state.status = value)),
  );
  actions.append(startButton, refreshButton, capabilityButton, summaryButton, stopButton);
  const errorSlot = document.createElement("div");
  statusCard.append(statusHeading, statusList, actions, errorSlot);

  const results = element("div", "result-grid");
  const capabilityResult = element("section", "card");
  const summaryResult = element("section", "card");
  results.append(capabilityResult, summaryResult);
  shell.append(header, statusCard, results);
  root.replaceChildren(shell);

  const render = (): void => {
    const connected = state.status.state === "connected";
    const startable = ["stopped", "crashed", "error"].includes(state.status.state);
    const busy = state.pending !== null;
    title.textContent = state.status.productName;
    version.textContent = `Development ${state.status.productVersion}`;
    badge.textContent = stateLabels[state.status.state];
    badge.dataset.state = state.status.state;
    runtimeVersion.textContent = state.status.runtimeVersion;
    generation.textContent = String(state.status.generation);
    statusDetail.textContent = state.status.detail;

    updateActionButton(
      startButton,
      state.pending === "Starting…" ? "Starting…" : "Start Runtime",
      busy || !startable,
      state.pending === "Starting…",
    );
    updateActionButton(
      refreshButton,
      state.pending === "Refreshing…" ? "Refreshing…" : "Refresh Status",
      busy,
      state.pending === "Refreshing…",
    );
    updateActionButton(
      capabilityButton,
      state.pending === "Querying…" ? "Querying…" : "Query Capabilities",
      busy || !connected,
      state.pending === "Querying…",
    );
    updateActionButton(
      summaryButton,
      state.pending === "Running…" ? "Running…" : "Run Mock Pipeline",
      busy || !connected,
      state.pending === "Running…",
    );
    updateActionButton(
      stopButton,
      state.pending === "Stopping…" ? "Stopping…" : "Stop Runtime",
      busy || !connected,
      state.pending === "Stopping…",
    );

    errorSlot.replaceChildren();
    if (state.error !== null) {
      const alert = element("p", "error", state.error);
      alert.setAttribute("role", "alert");
      errorSlot.append(alert);
    }

    renderCapability(capabilityResult, state.capability);
    renderSummary(summaryResult, state.summary);
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

function renderCapability(card: HTMLElement, capability: CapabilityDto | null): void {
  card.replaceChildren();
  card.append(element("h2", "", "Capabilities"));
  if (capability === null) {
    card.append(element("p", "empty", "Connect the Runtime, then query its bounded capability summary."));
    return;
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
}

function renderSummary(card: HTMLElement, summary: MockPipelineSummaryDto | null): void {
  card.replaceChildren();
  card.append(element("h2", "", "Latest Mock Summary"));
  if (summary === null) {
    card.append(element("p", "empty", "No deterministic native summary has been requested."));
    return;
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
}

function actionButton(activate: () => void): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.addEventListener("click", activate);
  return button;
}

function updateActionButton(
  button: HTMLButtonElement,
  label: string,
  disabled: boolean,
  busy: boolean,
): void {
  button.textContent = label;
  button.disabled = disabled;
  if (busy) button.setAttribute("aria-busy", "true");
  else button.removeAttribute("aria-busy");
}

function appendDefinition(list: HTMLDListElement, name: string): HTMLElement {
  const value = document.createElement("dd");
  list.append(element("dt", "", name), value);
  return value;
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
