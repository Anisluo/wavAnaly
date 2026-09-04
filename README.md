# wavAnaly — Surfer 二次开发（协议解码）

`surfer/` 是 https://gitlab.com/surfer-project/surfer 的浅克隆（v0.7.0，2026-09-04），
在其上增加了**多信号协议解码**功能：选 SCL / SDA 两根线，生成一条解码后的字符串信号，
在波形区直接显示 `S / 0x36 W / ACK / 0x0C / Sr / 0x36 R / NACK / P`。

## 构建 / 运行

Rust 工具链装在 `C:\Users\Admin\.cargo\bin`（stable, x86_64-pc-windows-gnu，链接器用 winget 的 mingw64）。

```bash
export PATH="/c/Users/Admin/.cargo/bin:$PATH"
cd wavAnaly/surfer
cargo build -p surfer            # 首次编译约 10 分钟
cargo run -p surfer -- ../../docs/timing/foc_bus.vcd
```

## 使用

1. 打开 VCD 后按 **空格** 打开命令行。
2. 输入：

```
decode_i2c i2c.SCL i2c.SDA
```

   可选第三个参数是显示名，默认 `i2c(SCL,SDA)`。若信号尚未加载，会先加载再自动解码。
3. 解码结果作为 `decoded.i2c(SCL,SDA)` 变量加入波形，可以像普通信号一样缩放、放游标、打 marker。

## 改动清单（相对上游）

| 文件 | 内容 |
|---|---|
| `libsurfer/src/decoders/mod.rs` | `VirtualSignal`（预计算的字符串信号）、`to_bit_trace`、`run(protocol, inputs)` 分发 |
| `libsurfer/src/decoders/i2c.rs` | I2C 状态机：SCL 高电平时 SDA 下降=START/上升=STOP；SCL 上升沿采样；第 9 位 ACK/NACK；含单元测试 |
| `libsurfer/src/wellen.rs` | `WellenContainer` 增加 `virtual_signals` 表；`query_variable` / `variable_to_meta` 优先查虚拟信号；`add_virtual_signal()` |
| `libsurfer/src/wave_container.rs` | 转发 `add_virtual_signal` / `is_virtual` |
| `libsurfer/src/message.rs` | 新消息 `DecodeProtocol { protocol, inputs, name }` |
| `libsurfer/src/lib.rs` | `decode_protocol()`：输入未加载时挂起到 `pending_decodes`，`SignalsLoaded` 后重试；解码完成后 `AddVariables` |
| `libsurfer/src/system_state.rs` | `pending_decodes` 字段 |
| `libsurfer/src/command_parser.rs` | `decode_i2c <scl> <sda> [name]` 命令 |

## 设计说明

上游的 Translator 是"单变量、单值、无状态"的，不能跨信号也看不到时间轴，所以协议解码
没法做成 translator（上游 issue #490 "Group translators" 正在讨论这个方向）。
这里绕开 translator，把解码结果做成容器层的**虚拟变量**：一次性离线跑完状态机，
之后绘制、游标、缩放全部复用现有代码。

## 后续可扩展

- `decoders/uart.rs`、`decoders/spi.rs`：在 `decoders::run` 里加分支，命令行加 `decode_uart <rx> <baud>`。
- 右键菜单：选中两条信号后 "Decode as I2C…"（`menus.rs::item_context_menu`）。
- 给 ACK/NACK/START 上色：解码段带 `ValueKind`，需要一个自定义 translator 替代 `StringTranslator`。
- 保存/恢复：虚拟信号目前不随 `save_state` 持久化，重新打开文件后需重新执行命令。
