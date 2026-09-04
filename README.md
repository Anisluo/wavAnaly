# wavAnaly

面向嵌入式与协议分析的波形查看器。基于 [Surfer](https://gitlab.com/surfer-project/surfer) 二次开发，
增加了**多信号协议解码**和**中文界面**，目标是把逻辑分析仪、示波器、HDL 仿真和固件打点产生的
VCD / FST / GHW 波形放在同一根时间轴上，并直接读出总线上传输的内容。

许可：EUPL-1.2（与上游一致），见 `LICENSE-EUPL-1.2.txt` 与 `NOTICE.md`。

## 功能

- Surfer 的全部能力：VCD / FST / GHW 加载、缩放、光标、标记、测量、分组、状态保存、WCP 远程控制、WASM 插件翻译器。
- **协议解码**：选定若干物理信号，生成一条解码后的字符串信号，与原始波形对齐显示。
  - `decode_i2c <scl> <sda> [名字]` — I²C：S / Sr / P、7 位地址 + 读写位、数据字节、ACK / NACK。
  - 规划中：UART、SPI、CAN、AHB-Lite / APB、单总线。
- **中文界面**：默认中文，`language = "en"` 或环境变量 `WAVANALY_LANG=en` 切回英文。
- **WaveDrom 脚本导入**：直接打开 `.json` / `.json5` / `.wavedrom` 文件（支持不带引号的键、单引号、注释、尾逗号），
  按 WaveDrom 语法生成波形显示；菜单 文件 → 导出 WaveDrom 为 VCD，或命令 `wavedrom_export_vcd <路径>`。
  扩展字段 `config.period_ns` 指定一个 WaveDrom 周期等于多少纳秒（默认 10）。示例见 `docs/timing/as5600_i2c.wavedrom.json`。

## 构建

需要 Rust 1.95+。Windows 上没有 MSVC 时可以用 GNU 工具链（`rustup default stable-x86_64-pc-windows-gnu`，需要 mingw64 在 PATH 中）。

```bash
cargo build --release -p wavanaly
./target/release/wavanaly your.vcd
```

## 使用

```bash
# 打开波形并执行命令
wavanaly foc_bus.vcd -C "variable_add i2c.SCL; variable_add i2c.SDA; decode_i2c i2c.SCL i2c.SDA"

# 或者从命令文件加载 (每行一条命令)
wavanaly amba_m0.vcd -c amba_m0.cmd
```

界面里按 **空格** 打开命令行，输入 `decode_i2c` 后按 Tab 补全信号名。解码结果会作为
`decoded.<名字>` 变量加入波形区，可以像普通信号一样缩放、放光标、打标记。

## 添加新的协议解码器

1. 在 `libsurfer/src/decoders/` 新建 `xxx.rs`，实现 `pub fn decode(inputs...) -> Vec<Segment>`。
   `Segment { time, text }` 表示从 `time` 开始显示 `text`，直到下一个 Segment。
2. 在 `decoders/mod.rs` 的 `run()` 里加分支，并把名字加进 `PROTOCOLS`。
3. 在 `command_parser.rs` 里加 `decode_xxx` 命令，把参数解析成 `Message::DecodeProtocol`。
4. 加单元测试（参考 `decoders/i2c.rs` 底部）。

解码器拿到的输入是每根线的 `(时间, 电平)` 变化列表（`BitTrace`），一次性离线处理整个波形，
不需要关心绘制、缩放和游标，这些由现有代码负责。

## 目录

| 目录 | 内容 |
|---|---|
| `surfer/` | 可执行程序 `wavanaly`（crate 名已改，目录名保留以便与上游合并） |
| `libsurfer/` | 主体库；`decoders/` 协议解码，`i18n/` 中文词典 |
| `surfer-translation-types/` | 翻译器插件 API |
| `surver/` | 远程波形服务器 |
| `docs/` | 上游文档 |

## 与上游的关系

上游 crate 名（`libsurfer` 等）、模块结构和命令名都保持不变，方便定期合并上游改动。
所有修改列在 `NOTICE.md` 和 `CHANGELOG.md` 里。
