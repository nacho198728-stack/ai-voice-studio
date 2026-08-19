# AI Voice Studio Phase 0.5 工程基础设施与 Runtime 骨架实施计划

## Objective

在不接入任何 AI 模型、Python Runtime、真实音频设备或用户业务功能的前提下，建立一个可在 Windows 与 macOS 编译、运行和验证的 Monorepo 骨架，打通 Tauri 2 桌面壳、Rust 控制面、C++20 独立 Runtime 进程、稳定 IPC 协议、VoiceEngine C ABI 和 Mock Engine 的完整控制链路。

本计划严格继承 V1.0 中的进程分层、故障隔离、实时线程约束和稳定 C ABI 原则，具体依据包括 `AI_Voice_Studio_技术立项与架构设计_V1.0.md:119`、`AI_Voice_Studio_技术立项与架构设计_V1.0.md:139`、`AI_Voice_Studio_技术立项与架构设计_V1.0.md:203`、`AI_Voice_Studio_技术立项与架构设计_V1.0.md:283` 和 `AI_Voice_Studio_技术立项与架构设计_V1.0.md:762`。

## Current Environment Baseline

- 当前系统为 macOS 26.5.2，Apple M5，arm64，内存 32 GiB。
- Xcode 26.6、macOS 26.5 SDK 和 Apple Clang 21.0.0 已安装，可提供 C++20 编译环境。
- Node.js 24.16.0、npm 11.13.0、Corepack 0.35.0、pnpm 11.19.0 和 Git 2.50.1 可用。
- Rust、Cargo、rustup、CMake、Ninja、pkg-config、ccache 和 Tauri CLI 当前不可用。
- 当前目录本身已经是 Git 仓库根目录，分支为 `main`；现有唯一项目文件是未跟踪的 V1.0 架构文档。
- 本机只能直接验证 macOS arm64；Windows 必须通过独立 Windows 环境或 CI runner 验证，不能用 macOS 交叉编译结果代替验收。

## Assumptions and Decisions Requiring Confirmation

- 将 `/Users/alex/Documents/ChatGPT/AI翻唱` 作为 Monorepo 根目录，直接创建 `apps/`、`core/`、`audio/` 等目录；需求中的 `AI-Voice-Studio/` 视为仓库概念名，不再创建嵌套 Git 仓库。
- `apps/runtime-host` 是 Rust library crate，由 Tauri 的 Rust backend 调用；它负责启动和管理独立的 C++ `voice-runtime` 子进程，不再增加一个常驻 Rust daemon。
- Rust 与 C++ Runtime 使用版本化、长度前缀的 UTF-8 JSON 消息经子进程标准输入/标准输出通信；C++ 日志走标准错误和文件，避免与 IPC 混流。协议 envelope 保持稳定，未来可把传输替换为 Windows Named Pipe 和 macOS Unix Domain Socket。
- VoiceEngine C ABI 位于 C++ Runtime 与模型插件之间。Rust 不跨进程直接解引用 C ABI；完整链路是 Tauri → Rust RuntimeManager → IPC → C++ Runtime → VoiceEngine C ABI → Mock Engine。这一边界避免把模型动态库加载到 UI/控制进程。
- Phase 0.5 不打开真实音频设备。Mock Pipeline 的 PCM 在 C++ Runtime 内生成和处理，IPC 只返回帧数、校验值、模拟延迟和指标，WebView 不接触 PCM。
- 初始依赖保持最小：Rust 使用 Tauri、Serde、Tokio 和 tracing；C++ 使用固定版本的 spdlog 与 JSON 库；测试优先使用 CTest 和 Rust 原生测试，避免引入大规模测试框架。
- 项目采用锁定版本与可重现构建策略；pnpm 通过 Corepack 和根 `packageManager` 字段固定，不依赖当前 Codex fallback 路径。
- `docs/adr` 从 Monorepo 初始化时即建立；重大决策与对应实现同批完成。Phase 0.5 使用 ADR-000 至 ADR-002，ADR-003 编号保留给 Phase 1 Audio Engine。

## Implementation Plan

- [x] 建立 Monorepo 顶层模块与仓库元数据。创建 `apps`、`core`、`audio`、`runtime`、`engines`、`backend`、`tests`、`docs` 和 `tools`，新增根 `README.md`、`.gitignore`、版本与许可证说明；现有 V1.0 文档保持原位且不覆盖。

- [x] 建立 pnpm 与 Cargo workspace。`pnpm-workspace.yaml` 和 `package.json` 管理 Desktop 与统一开发命令；根 `Cargo.toml` 管理 Tauri backend、`apps/runtime-host` 和 Rust contracts，且不复制 CMake 内部逻辑。

- [ ] 建立 CMake 工程与 presets。根 `CMakeLists.txt` 和 `CMakePresets.json` 管理 C++ Runtime、C ABI、Mock Engine 与 CTest，并定义跨平台产物目录。

- [ ] 固定工具链与开发环境契约。声明支持的 Node、pnpm、Rust stable、CMake、C++20、Xcode 和 Visual Studio 版本，提供只检查不修改机器的 doctor 脚本，并在开发文档中分别给出 macOS 与 Windows 的官方安装来源。受影响路径包括 `.node-version`、`rust-toolchain.toml`、`tools/scripts` 和 `docs/development/DEVELOPMENT.md`。这样可以避免开发者依赖当前机器的隐式路径或全局 Tauri CLI。

- [ ] 定义跨语言版本与 ErrorCode。`core/contracts/version.json` 记录 Runtime、protocol、VoiceEngine ABI 和兼容规则；`error-codes.json` 固定错误名称、数值和语义，再映射到 Rust/C++ 类型并检测漂移。

- [ ] 定义第一版 VoiceEngine C ABI 及其合同。`runtime/api/voice_engine.h` 覆盖规定的八类操作，结构携带 ABI version 与 struct size；ADR 明确 caller-owned buffer、编码、错误/异常边界、生命周期和线程规则，prepare 后的处理调用禁止分配、锁和日志。

- [ ] 实现 C++20 `voice-runtime` 独立进程骨架。Runtime 启动后输出版本化握手，进入命令循环，支持 ping、get_capabilities、run_mock_pipeline、shutdown 与统一错误返回；进程不打开音频设备、不加载 AI Runtime。为后续故障恢复保留退出原因、generation 和健康状态。受影响路径包括 `runtime/session`、`runtime/scheduler`、`runtime/loader`、`backend/mock` 和 CMake targets。

- [ ] 实现 Mock VoiceEngine 动态插件。Mock Engine 通过唯一的 C ABI 工厂入口暴露函数表，支持确定性初始化、模拟模型加载、流准备、可配置处理延迟、固定且可校验的 PCM 转换、reset 和 metrics；它不得引用 Rust/Tauri 或任何 AI/音频设备依赖。受影响路径包括 `engines/mock` 和 `backend/mock`。确定性输出让端到端测试可以验证确实调用了插件，而不是只验证 Runtime 存活。

- [ ] 定义 RuntimeMessage 并实现有界 IPC framing。schema 固定 version、request_id、command、payload 和 result/ErrorCode；Rust/C++ 一致处理版本、截断、粘包和大小上限。C++ stdout 专用于协议，日志使用 stderr/文件。

- [ ] 实现 Rust RuntimeManager 状态机。提供 start_runtime、stop_runtime、get_runtime_status 和 get_capabilities，并增加内部的握手、request correlation、超时、优雅关闭、进程退出监控、标准错误收集和崩溃恢复接口；状态至少包括 stopped、starting、connected、stopping、crashed 和 error。受影响路径包括 `apps/runtime-host`。Phase 0.5 只建立有限重启策略接口，不实现无限自动重启。

- [ ] 建立统一日志结构。Rust 使用 tracing，C++ 使用 spdlog，统一 timestamp、component、level、message 和可选 request_id/generation 字段；默认写入开发日志目录，DEBUG 仅由配置打开，Mock 的 process_audio 热路径只更新原子指标而不逐帧打印。受影响路径包括 `core/telemetry`、`apps/runtime-host`、C++ Runtime 日志模块和 `config/config.json`。

- [ ] 建立最小配置与能力模型。创建版本化 `config/config.json`，包含 runtime、audio 占位设置、backend=mock 和 debug；定义 Hardware/Runtime/Engine capability 的最小结构，但 Phase 0.5 只返回平台、架构、Runtime version、Mock backend 和 Mock Engine 能力，不进行硬件 Benchmark。受影响路径包括 `core/capability`、`core/model-package`、`core/telemetry` 和配置加载模块。

- [ ] 创建 Desktop Shell 并打通 Mock 控制链路。`apps/desktop` 只显示产品名、开发版本、Runtime 状态和控制操作；Tauri 经 RuntimeManager/IPC 触发 C++ 内部 Mock PCM，UI 只接收帧数、校验值、耗时和 metrics，不接触 PCM 或业务功能。

- [ ] 在 `tests/contract` 建立 C ABI contract test。分别用 C 和 C++ 编译头文件，验证 ABI version、struct size、函数表和插件入口。

- [ ] 在 Rust workspace 建立协议 contract test。验证 RuntimeMessage、ErrorCode、版本兼容和非法消息拒绝行为。

- [ ] 在 `tests/runtime` 建立进程集成测试。覆盖启动、握手、能力查询、优雅关闭、异常退出和无孤儿进程。

- [ ] 在 `tests/benchmark` 建立 Mock Pipeline 测试。验证确定性输出、模拟延迟、reset、metrics 和返回校验值。

- [ ] 在 `.github/workflows/build.yml` 新增 macOS arm64 与 Windows x64 jobs。两个干净 runner 均执行 CMake/CTest、Cargo、前端和 Tauri 验证；Windows 额外验证 MSVC 动态库加载与子进程关闭，真实 Windows 结果为验收门槛。

- [ ] 完成工程文档与架构决策历史。生成 `docs/development/DEVELOPMENT.md`、`docs/architecture/ARCHITECTURE.md`、`docs/adr/ADR-000-monorepo.md`、`ADR-001-tauri-rust-control-plane.md` 和 `ADR-002-cpp-runtime-c-abi.md`；每份 ADR 记录背景、决定、理由、替代方案、后果和状态，并与对应实现同批完成。ADR-003 明确保留给 Audio Engine。

- [ ] 执行 Phase 0.5 最终验收并冻结基线。按需求清单逐项记录 macOS 本机结果和 Windows CI 结果，确认安装包/进程不包含 Python、PyTorch、CUDA、AI 模型或真实音频访问；记录已知限制、构建产物位置、版本和下一阶段输入。验收报告放在 `docs/development/PHASE-0.5-ACCEPTANCE.md`，只有所有硬门槛通过才标记完成。

## Verification Criteria

- 当前 Git 仓库直接形成约定的 Monorepo 结构，现有 V1.0 文档保持完整且无无关改动。
- macOS arm64 和 Windows x64 的干净环境均能完成 CMake、Cargo、前端和 Tauri 构建；Windows 结果来自真实 runner。
- Desktop 窗口能够显示 Runtime 未启动、启动中、已连接、停止中和错误/崩溃状态。
- Rust RuntimeManager 能启动、握手、查询、优雅停止 C++ `voice-runtime`，超时或崩溃后不遗留孤儿进程。
- IPC 对截断帧、超大 payload、未知 command、request_id 错配和 ABI/protocol major 不匹配返回确定的 ErrorCode。
- C++ Runtime 能动态加载 Mock Engine，并且 Mock Pipeline 的输出校验值证明 process_audio 已经通过 C ABI 执行。
- `voice_engine.h` 可由纯 C 与 C++20 translation unit 使用；ABI version、struct size、内存所有权、错误码和线程要求均有自动测试和文档。
- UI、Rust IPC 和日志中不传输 Mock PCM；PCM 只存在于 C++ Runtime 的测试管线内。
- Rust 与 C++ 日志均包含 timestamp、component、level、message，标准输出的 IPC 不被日志污染，热路径无逐帧日志。
- `config/config.json` 能被加载和校验，未知字段遵循兼容策略，错误配置产生可行动错误而非崩溃。
- Contract Test、Runtime Test、Mock Pipeline Test 和跨平台 Build Test 全部通过。
- ADR-000、ADR-001、ADR-002 均存在并与实际代码一致；重大决策不会只留在聊天或提交说明中，ADR-003 未被 Phase 0.5 占用。
- 代码和构建图中不存在 Python、PyTorch、CUDA、ONNX Runtime、真实 AI 模型、账号、云服务、播放器或真实音频设备接入。

## Potential Risks and Mitigations

1. **本机缺少 Rust、CMake 和 Ninja，当前无法开始构建。**
   Mitigation: 获得确认后仅从官方 rustup 和已验证包源安装锁定工具；安装后记录版本、路径和是否需要新 shell，并先运行 doctor 再创建工程。

2. **当前 pnpm 来自 Codex bundled fallback 路径，不适合作为项目隐式依赖。**
   Mitigation: 通过 Corepack 和根 `packageManager` 字段固定 pnpm，CI 与开发文档均从声明恢复版本。

3. **当前仓库仅有未跟踪的 V1.0 文档且没有提交历史，误建嵌套仓库会造成版本边界混乱。**
   Mitigation: 默认使用当前 Git 根作为 Monorepo 根，不运行嵌套 `git init`，所有创建前再次检查状态并保留用户文件。

4. **需求示意为 Rust → C ABI，但独立 C++ Runtime 进程与 C ABI 不能直接跨进程调用。**
   Mitigation: 明确采用 Rust → IPC → C++ Runtime → C ABI → Engine；C ABI 仅用于 Runtime 内插件边界，ADR 和端到端测试共同固定这一事实。

5. **标准输入/输出 IPC 未来可能不满足多客户端、断线重连或大消息。**
   Mitigation: Phase 0.5 只承载小型控制消息，协议 envelope 与 transport 解耦；后续换 Named Pipe/Unix Socket 时保持 RuntimeMessage 不变。

6. **日志写入标准输出会破坏 IPC framing。**
   Mitigation: C++ stdout 专用于 IPC，spdlog 只写 stderr/文件；测试主动注入日志并验证协议不受污染。

7. **C ABI 在编译器、架构和版本间发生布局或所有权漂移。**
   Mitigation: 只使用固定宽度 C 类型、opaque handle、caller-owned buffer、struct_size 和 abi_version；禁止异常/STL/Rust 类型跨边界，并在 C/C++/Windows/macOS 做 contract test。

8. **Tauri backend 与独立 Rust RuntimeManager 重复或形成多余进程。**
   Mitigation: `apps/runtime-host` 设计为可测试 library crate，由 Tauri backend 组合使用；唯一额外常驻进程是 C++ `voice-runtime`。

9. **Mock Pipeline 为追求“完整链路”而把 PCM 送进 WebView/Rust，破坏长期边界。**
   Mitigation: 测试 PCM 在 C++ Runtime 内生成，UI/Rust 只收控制和摘要指标；测试断言响应不含 PCM 字段。

10. **Windows 只在 CI 编译但没有实际 GUI/进程生命周期验证。**
    Mitigation: CI 至少运行 headless Runtime/contract/mock tests；Phase 0.5 关闭前在真实 Windows x64 主机完成一次 Tauri 启动与进程控制 smoke test并留存结果。

11. **C++ 第三方依赖下载影响可重现性和供应链。**
    Mitigation: 使用官方仓库、固定版本与哈希，生成依赖清单；不接受浮动 main/master，后续可转入受控 vendor cache。

12. **过早实现 scheduler/audio/model schema 导致范围蔓延。**
    Mitigation: scheduler、audio 和 model-package 在本阶段只建立接口/占位职责；验收明确禁止真实设备、AI Runtime、模型解析和性能优化。

## Alternative Approaches

1. **在当前仓库下再创建 `AI-Voice-Studio/` 子仓库：** 路径与需求图完全一致，但会形成嵌套工程、分散现有 V1.0 文档并增加 Git/CI 工作目录复杂度。除非用户明确要求，暂不采用。

2. **Rust 直接动态加载 Mock Engine C ABI：** 链路更接近需求示例，但会把模型插件加载到 Tauri/Rust 控制进程，削弱崩溃隔离并使未来真实 Runtime 边界失真，不采用。

3. **Phase 0.5 直接使用 Named Pipe 与 Unix Domain Socket：** 更接近长期生产传输，但会提前增加双平台端点权限、命名、清理和重连复杂度。当前选择 transport-neutral framing over stdio，后续保持协议替换传输。

4. **使用 Protobuf/gRPC：** schema 与代码生成成熟，但为少量本机控制消息引入较大 C++/Rust/Tauri 构建与依赖成本。若未来多客户端、远程诊断或高频二进制控制成为需求，可通过 ADR 重新评估。

5. **使用 JUCE 建立本阶段 Audio Engine：** 能更早验证设备，但违反“不连接真实音频”和控制范围的要求，同时引入商业许可与依赖；本阶段只保留 `audio` 模块边界，延后到正式 Audio Harness 阶段。

6. **使用 Electron 或把 Mock DSP 放进 WebAudio：** 开发简单但直接违反 Tauri 与实时边界要求，不考虑。
