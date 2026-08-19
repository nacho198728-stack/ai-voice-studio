# AI Voice Studio Phase 1 Native Runtime 与 Audio Engine 实施计划

## Objective

在 Phase 0.5 工程骨架真实存在并通过验收后，建设独立 C++20 Native Runtime 的实时音频基础设施，打通 macOS CoreAudio 输入、固定内存实时管线、Mock DSP 和物理输出；同时保留 Windows WASAPI/ASIO 的平台边界，使未来 VoiceEngine 插件只消费统一 AudioBlock，而不重新设计设备、线程、Buffer、时钟、Telemetry 和恢复机制。

本计划不接入 MNP-SVC、MeanVC2、RVC、ONNX Runtime、Core ML、TensorRT、CUDA、Python、虚拟麦克风、账号、云端或模型业务。

架构依据为：进程与 PCM 边界见 `AI_Voice_Studio_技术立项与架构设计_V1.0.md:147` 和 `AI_Voice_Studio_技术立项与架构设计_V1.0.md:156`；实时数据流、线程和数据结构见 `AI_Voice_Studio_技术立项与架构设计_V1.0.md:203`、`AI_Voice_Studio_技术立项与架构设计_V1.0.md:222` 和 `AI_Voice_Studio_技术立项与架构设计_V1.0.md:236`；背压和恢复原则见 `AI_Voice_Studio_技术立项与架构设计_V1.0.md:245`；VoiceEngine 插件合同见 `AI_Voice_Studio_技术立项与架构设计_V1.0.md:283`。

## Current Code Review

- 当前 Git 仓库没有任何 tracked file，也没有 commit 基线。
- 工作区只有 V1.0、Phase 0.5 计划和 `.DS_Store`，不存在任何源码、构建清单、依赖锁文件或 CI。
- Phase 0.5 的全部实现任务仍是未勾选计划项，见 `plans/2026-08-19-phase-0-5-infrastructure-v1.md:28`；其最终验收任务也未执行，见 `plans/2026-08-19-phase-0-5-infrastructure-v1.md:68`。
- Rust、Cargo、CMake 和 Ninja 不可用；Node 与 pnpm 可用。当前没有编译入口，因此不能声称 Phase 0.5 或 Phase 1 可编译。
- 目前可复用的只有 V1.0 架构约束和 Phase 0.5 计划中的拟议边界：独立 RuntimeManager、稳定 IPC、C ABI、Mock Engine、日志、配置和测试；没有可复用实现。

## Entry Gate and Assumptions

- **硬入口门禁：** 必须先执行并验收 `plans/2026-08-19-phase-0-5-infrastructure-v1.md`。在 Tauri → Rust → IPC → C++ Runtime → C ABI → Mock Engine 未打通前，不创建真实音频设备代码。
- 当前仓库根 `/Users/alex/Documents/ChatGPT/AI翻唱` 作为 Monorepo 根，不建立嵌套 Git 仓库。
- 优先采用 V1.0 推荐的 C++20 + JUCE audio modules，并通过公司商业许可证分发；内部 `PlatformAudioBackend` 隔离 JUCE 类型。若 JUCE 商业授权未确认，Phase 1 只能停留在内部验证，不能外部分发测试包。
- macOS 实现真实 CoreAudio backend；Windows 本阶段提供能编译和测试状态/错误路径的 Mock backend，真实 WASAPI/ASIO 延后。
- 设备侧优先 48 kHz float32。不同输入/输出 clock domain 不能假定同步：worker 侧预留并验证 clock drift controller 与 stateful asynchronous resampler；callback 不做重采样。
- 真正物理 round-trip latency 需要线缆回环或可信 loopback 设备。没有物理回环时只报告 device-reported、callback、buffer 和 processing latency，禁止把估算值标成端到端实测。
- 两小时稳定性分为可自动化的 Mock/Null backend soak 和需要麦克风权限、耳机及真实设备的 CoreAudio soak；二者都通过才关闭 Phase 1。

## Implementation Plan

- [ ] 先完成 Phase 0.5 入口审计。确认 Monorepo、工具链、CMake/Cargo/pnpm、Tauri、Rust RuntimeManager、C++ `voice-runtime`、IPC、VoiceEngine C ABI、Mock Engine 和跨平台基础测试全部存在并通过；把结果记录到 `docs/development/PHASE-0.5-ACCEPTANCE.md`，否则停止 Phase 1。

- [ ] 冻结 Phase 1 依赖与许可决策。固定 JUCE、spdlog、JSON 库和测试依赖的版本、来源、哈希与许可证；在 `docs/adr/ADR-003-audio-engine-design.md` 记录 JUCE commercial/AGPL 边界以及未获商业授权时禁止外部分发。

- [ ] 扩展音频 contracts 与配置。为 device、stream format、buffer、latency、metrics、recovery 和 Audio ErrorCode 定义版本化结构；更新 `config/config.json` 与 IPC schema，明确 runtime process lifecycle 和 audio stream lifecycle 是两套不同状态。

- [ ] 完善 `voice-runtime` 状态机。实现 Created、Initializing、Ready、Running、Stopping、Stopped、Error 的显式转移表、generation、last error 和幂等 stop/shutdown；非法转换返回 INVALID_STATE，所有资源释放发生在非 callback 线程。

- [ ] 定义 `PlatformAudioBackend` 与 CoreAudio 能力边界。接口覆盖设备枚举、能力查询、打开、启动、停止、关闭、延迟和 callback 注册；平台类型不得泄漏到 `audio/engine`，Windows 预留 WASAPI/ASIO factory。

- [ ] 实现 macOS CoreAudio backend。通过 JUCE CoreAudio 路径完成设备枚举、稳定 UID、默认状态、采样率/通道/buffer 能力、输入输出启动和安全停止；首次启动静音并渐入，避免扬声器反馈和设备打开爆音。

- [ ] 建立 Windows Mock backend。Windows x64 构建能枚举虚拟测试设备、模拟 start/stop/device lost，并证明 AudioEngine 不依赖 macOS 类型；真实 WASAPI 与 ASIO 不进入本阶段。

- [ ] 实现固定布局 AudioBlock 与预分配 Slot Pool。Header 包含规定的 sample rate、channels、frames、timestamp、sample index、format 和 discontinuity；样本采用对齐的 planar float32 存储，Block 只移动 handle/view，不隐式复制或跨 IPC。

- [ ] 实现输入与输出 SPSC Ring。固定容量和启动前内存分配，使用单调读写序号与清晰 memory ordering；统计 level、overflow、underflow 和 dropped telemetry，策略明确区分输入追实时丢旧块与输出缺块安全静音。

- [ ] 建立四类线程与实时违规防线。Audio Callback 仅复制、时间戳和原子计数；Worker 组帧、DSP、时钟桥接；Control 处理 IPC/状态；Telemetry 聚合指标。Debug 构建提供 callback 分配、锁、日志和 deadline violation 检测。

- [ ] 实现 AudioEngine 生命周期和设备管理。统一 initialize/start/stop/shutdown、enumerateDevices、getCapabilities、getLatency、getMetrics 和设备选择；切换设备必须 stop → close → enumerate/open → prepare → start，并带 generation 防止旧 callback 写入新会话。

- [ ] 实现时钟漂移与格式桥接。检测输入/输出 clock domain 和 sample position 偏差，在 Worker 使用 ring 水位反馈调节 stateful asynchronous resampler ratio；以合成 ±100 ppm 漂移验证两小时内不累积 underflow/overflow，callback 不执行 SRC。

- [ ] 实现 Mock DSP Pipeline。Gain 与 artificial compute delay 运行在 Worker，支持 10/30/50/100 ms；另行区分固定音频 delay line 与模拟处理耗时，确保报告能判断是算法延迟还是 deadline overload。

- [ ] 实现非侵入式 Telemetry。Callback/Worker 只向固定容量 metrics queue 写小型样本或原子计数，Telemetry thread 聚合 callback duration/miss、ring level、xrun、processing p50/p95/p99、进程 CPU 和 RSS，并以版本化 JSON 低频输出。

- [ ] 建立 Latency Benchmark 与报告。`tests/benchmark/audio_latency` 生成包含 OS、device UID/name、sample rate、buffer、测量方法、平均值、p50/p95/p99 和 xrun 的报告；区分 reported、internal measured 和 physical loopback 三种延迟口径。

- [ ] 实现设备和 Runtime 恢复流程。覆盖设备拔出、默认设备变化、采样率/buffer 变化、callback 异常、启动失败和 Runtime 退出；Control thread 负责释放和重枚举，callback 只设置 fault flag，恢复有退避、次数上限和可观察错误。

- [ ] 扩展 Rust RuntimeManager 与 IPC。增加 initialize_audio、start_audio、stop_audio、list_audio_devices、select_audio_device、audio_status 和 metrics；Rust 只处理控制与 JSON 指标，PCM、AudioBlock pointer 和 callback 数据不得进入 IPC。

- [ ] 建立单元、集成与压力测试。覆盖 AudioBlock layout/lifetime、SPSC 顺序/绕回/溢出、状态机、Mock backend、Mock DSP 延迟、IPC、设备切换、随机 callback size、CPU 干扰、device lost 和 Runtime 异常退出；测试必须能确定性重现并输出 seed。

- [ ] 执行真实设备与两小时稳定性验收。先完成 Mock/Null backend 自动 soak，再在 macOS 对可用的内置麦克风、耳机、USB 麦克风或声卡逐项测试；记录未连接设备，不伪造结果。验收要求无崩溃/死锁/持续内存增长，xrun 和恢复结果按设备报告。

- [ ] 完成跨平台 CI、文档和 Phase 1 验收。macOS runner 构建 CoreAudio backend 并运行非权限测试，Windows runner 构建 Mock backend；新增 `AUDIO_RUNTIME.md`、`TEST_PLAN_AUDIO.md`、ADR-003 和 `PHASE-1-ACCEPTANCE.md`，逐项关闭架构、音频、性能、稳定性和工程清单。

## Expected File Changes

Phase 0.5 未实现，因此以下是条件式预计路径；实际执行时先遵循 Phase 0.5 建立的命名和 conventions：

- 根构建与配置：`CMakeLists.txt`、`CMakePresets.json`、`config/config.json`、`.github/workflows/build.yml`。
- Runtime：`runtime/session` 下的 runtime state/status/controller，`runtime/scheduler` 下的 audio worker 调度，`runtime/loader` 的 Mock Engine 连接，Runtime command handler 与 executable entry。
- 音频接口：`audio/engine` 下的 AudioEngine、AudioConfig、AudioStatus、DeviceManager 和 ClockDomainBridge。
- Buffer：`audio/buffers` 下的 AudioBlock、AudioBufferSlot、BlockPool、SpscRing 和指标结构。
- DSP：`audio/dsp` 下的 ProcessingPipeline、Gain、ArtificialDelay、AsyncResampler interface 与 Mock Pipeline。
- 平台：`audio/platform` 下的 PlatformAudioBackend/factory，`audio/platform/macos` 的 CoreAudio adapter，`audio/platform/windows` 的 Mock/WASAPI/ASIO placeholders。
- Telemetry：`core/telemetry` 的 audio metrics、histogram snapshot、process metrics 和 JSON serialization。
- Contracts：`core/contracts` 的 audio commands、status、capabilities、ErrorCode 和 protocol schema；对应 Rust/C++ 类型。
- Rust：`apps/runtime-host` 的 audio control methods、IPC request/response 和 Runtime status integration。
- 测试：`tests/contract`、`tests/runtime`、`tests/audio`、`tests/stress`、`tests/benchmark/audio_latency` 及 CTest/Cargo targets。
- 文档：`docs/adr/ADR-003-audio-engine-design.md`、`docs/architecture/AUDIO_RUNTIME.md`、`docs/development/TEST_PLAN_AUDIO.md`、`docs/development/PHASE-1-ACCEPTANCE.md`。

## Verification Criteria

- Phase 0.5 的真实代码、构建和验收报告存在；不是仅有计划文档。
- `voice-runtime` 与 AudioEngine 状态转移全部可测试，非法调用返回统一错误且不会卡死。
- AudioEngine 不包含 JUCE/CoreAudio/WASAPI/ASIO 具体类型，平台 backend 可由 factory 替换。
- macOS arm64 能枚举真实设备、打开输入输出、通过 Mock DSP 输出，并能停止和重复启动。
- Audio Callback 在测试 instrumentation 下无动态分配、mutex、文件/网络 I/O、日志和阻塞等待。
- AudioBlock 固定布局、显式生命周期、无隐式复制；PCM 不通过 Rust、Tauri 或 IPC。
- SPSC Ring 在并发、绕回、overflow/underflow 和长时测试下无数据竞争或越界。
- 10/30/50/100 ms 模拟处理能产生可解释的 buffer/overload 行为，不能靠无界缓存隐藏延迟。
- Telemetry 输出 callback、deadline miss、ring、xrun、processing p50/p95/p99、CPU 和 RSS，且采集不会阻塞实时线程。
- Latency Report 标明测量口径；没有物理 loopback 时不声称已测真实 round-trip latency。
- 设备丢失、不可用、格式变化与 Runtime 异常不会造成永久 Running、死锁或孤儿设备。
- Rust 可控制 audio start/stop/status/metrics，协议中不存在 PCM 字段。
- Mock/Null backend 与真实 CoreAudio 均完成两小时 soak；无崩溃、死锁和持续内存增长。
- macOS CoreAudio 与 Windows Mock backend 均通过 CI build/test；Windows 不误标为真实音频支持。
- 代码依赖图不含任何禁止的 AI、Python、GPU、云端、账号或虚拟驱动组件。

## Potential Risks and Mitigations

1. **Phase 0.5 实际缺失，Phase 1 没有可扩展基线。**
   Mitigation: 将 Phase 0.5 全部验收设为硬门禁；不在空仓库里直接堆叠音频代码。

2. **JUCE 商业许可尚未确认。**
   Mitigation: 采用内部 abstraction 并在 ADR/SBOM 记录许可；未获得商业许可前不向外部分发闭源测试包。若拒绝 JUCE，则先重新评审 native CoreAudio/WASAPI 的额外成本。

3. **macOS 麦克风 TCC 权限无法由自动测试自行授予。**
   Mitigation: CI 使用 Mock/Null backend；真实设备测试由签名开发包触发明确授权，并在验收报告记录权限状态。

4. **输入和输出属于不同 clock domain，长时运行必然漂移。**
   Mitigation: 不能只靠增大 ring；在 Worker 实现漂移估计和异步 SRC，并用 ±100 ppm 合成测试验证。

5. **CoreAudio 热插拔回调与资源销毁竞态导致 use-after-free。**
   Mitigation: callback 只发布 fault/generation，Control thread 串行执行 stop/close/reopen；对象生命周期用 RAII 和 callback drain barrier 管理。

6. **人工 100 ms 处理延迟让 10 ms hop 永久过载。**
   Mitigation: 报告明确这是预期 overload test；区分 compute delay 与 audio delay line，不通过增加无界队列伪装实时。

7. **Telemetry 自身引入锁、分配或 cache contention。**
   Mitigation: 热线程只写固定指标队列/原子值，分位数与 JSON 在低优先级线程聚合；记录 telemetry drop count。

8. **真实 round-trip latency 无硬件回环便无法准确测量。**
   Mitigation: 报告分离 reported/internal/physical 三种来源；正式 p50/p95/p99 端到端结论必须有物理 loopback。

9. **扬声器输出回灌麦克风导致啸叫或测试伤害听力。**
   Mitigation: 首次启动默认静音、限制增益并渐入；真实测试要求耳机，未检测/确认安全路由时不自动打开监听。

10. **两小时真实设备测试受休眠、电源和设备缺失影响。**
    Mitigation: 禁止休眠并记录电源状态；可用设备逐项实测，缺失的 USB/声卡明确列为未验证，不以 Mock 替代。

11. **Windows 只有 Mock backend，跨平台音频风险仍未关闭。**
    Mitigation: Phase 1 只宣称接口和构建可扩展；真实 WASAPI/ASIO 单列后续里程碑，不能写入本阶段完成项。

12. **AudioBlock 固定 ABI 过早冻结会限制未来模型帧长和声道。**
    Mitigation: 固定 descriptor 布局但把样本容量放在预分配 slot/config 中，使用 struct_size/version 扩展，避免在 ABI 中嵌入超大静态数组。

## Alternative Approaches

1. **跳过 Phase 0.5，直接创建 Audio Engine：** 速度表面更快，但 IPC、状态、构建、ABI 和测试都会在音频模块内临时发明，直接违反阶段化架构，不采用。

2. **不用 JUCE，直接编写 CoreAudio/WASAPI：** 控制力更强且没有 JUCE 商业许可，但需要长期维护双平台设备、热插拔、buffer 和 ASIO 边界。仅在明确拒绝 JUCE 并接受额外周期后采用。

3. **使用纯 Rust cpal：** 类型安全和 Tauri 集成方便，但与 V1.0 的 C++/JUCE/ONNX/TensorRT Runtime 方向不一致，专业设备与 ASIO 的产品风险更高，不作为主线。

4. **Audio Callback 直接执行 Mock/未来 VoiceEngine：** 可以减少一次 ring hop，但模型耗时抖动会阻塞设备 callback；本阶段坚持 callback 与 Worker 隔离。

5. **一个环形缓冲同时承载输入和输出：** 状态简单但生产者/消费者关系不再是严格 SPSC，故障和水位语义混乱；采用独立 input/output rings。

6. **只依赖 JUCE/驱动报告的 latency：** 易实现但无法代表完整链路；保留内部 sample-index 测量与可选物理 loopback。
