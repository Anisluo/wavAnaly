//! 中文词典: zh_help
//!
//! 覆盖 help.rs（快速上手/控制/许可证/关于窗口）、dialog.rs（重新加载/发现状态文件对话框）、
//! logs.rs（日志窗口标题与按钮）、file_dialog.rs（文件对话框标题）。

pub const ENTRIES: &[(&str, &str)] = &[
    // help.rs - 启动提示
    (
        "Drag and drop a VCD, FST, or GHW file here to open it",
        "将 VCD、FST 或 GHW 文件拖放到此处以打开",
    ),
    (
        "Or press {} and type load_url",
        "或按 {} 并输入 load_url",
    ),
    (
        "Or press {} and type load_file or load_url",
        "或按 {} 并输入 load_file 或 load_url",
    ),
    (
        "Or use the file menu or toolbar to open a URL",
        "或使用文件菜单或工具栏打开一个 URL",
    ),
    (
        "Or use the file menu or toolbar to open a file or a URL",
        "或使用文件菜单或工具栏打开一个文件或 URL",
    ),
    ("Or click", "或点击"),
    ("here", "这里"),
    ("to open an example waveform", "打开一个示例波形"),
    ("Recent files", "最近打开的文件"),
    (
        "Note that this web based version is a bit slower than a natively installed version. There may also be a long delay with unresponsiveness when loading large waveforms because the web assembly version does not currently support multi threading.",
        "请注意，此网页版本比原生安装版本略慢。由于 WebAssembly 版本目前不支持多线程，加载大型波形时可能会出现较长时间的无响应。",
    ),
    // help.rs - 关于窗口
    ("About wavAnaly", "关于 wavAnaly"),
    ("🏄 wavAnaly", "🏄 wavAnaly"),
    ("Cargo version: {}", "Cargo 版本：{}"),
    ("Git version: {}", "Git 版本：{}"),
    ("Click to copy git version", "点击复制 Git 版本号"),
    ("Build date: {}", "构建日期：{}"),
    (" repository", " 仓库"),
    ("Homepage", "主页"),
    ("Close", "关闭"),
    // help.rs - 快速上手窗口
    ("🏄 wavAnaly quick start", "🏄 wavAnaly 快速上手"),
    ("Controls", "控制方式"),
    (
        "↔ Use scroll and ctrl+scroll to navigate the waveform",
        "↔ 使用滚轮和 Ctrl+滚轮浏览波形",
    ),
    (
        "🚀 Press {} to open the command palette",
        "🚀 按 {} 打开命令面板",
    ),
    (
        "✋ Click the middle mouse button for gestures",
        "✋ 点击鼠标中键以使用手势",
    ),
    ("❓ See the help menu for more controls", "❓ 查看帮助菜单以获取更多控制方式"),
    ("Adding traces", "添加信号"),
    (
        "Add more traces using the command palette or using the sidebar",
        "使用命令面板或侧边栏添加更多信号",
    ),
    ("Opening files", "打开文件"),
    ("Open a new file by", "通过以下方式打开新文件"),
    ("- dragging a VCD, FST, or GHW file", "- 拖放 VCD、FST 或 GHW 文件"),
    (
        "- typing load_url in the command palette",
        "- 在命令面板中输入 load_url",
    ),
    (
        "- typing load_url or load_file in the command palette",
        "- 在命令面板中输入 load_url 或 load_file",
    ),
    ("- using the file menu", "- 使用文件菜单"),
    ("- using the toolbar", "- 使用工具栏"),
    // help.rs - 控制窗口标题
    ("🖮 wavAnaly controls", "🖮 wavAnaly 控制方式"),
    // help.rs - 按键说明表
    ("Show command prompt", "打开命令行"),
    ("Pan", "平移"),
    ("Zoom", "缩放"),
    ("Save the state", "保存当前状态"),
    ("Show or hide the design hierarchy", "显示或隐藏设计层级树"),
    ("Show or hide menu", "显示或隐藏菜单"),
    ("Show or hide toolbar", "显示或隐藏工具栏"),
    ("Zoom in", "放大"),
    ("Zoom out", "缩小"),
    ("Zoom in on cursor", "以光标为中心放大"),
    ("UI Zoom in", "放大界面"),
    ("UI Zoom out", "缩小界面"),
    ("Scroll up", "向上滚动"),
    ("Scroll down", "向下滚动"),
    ("Move focused item up", "将焦点项上移"),
    ("Move focused item down", "将焦点项下移"),
    ("Move focus up", "焦点上移"),
    ("Move focus down", "焦点下移"),
    ("Add focused item to selection", "将焦点项加入选择"),
    ("Extend selection up", "向上扩展选择"),
    ("Extend selection down", "向下扩展选择"),
    ("Undo last change", "撤销上一次更改"),
    ("Redo last change", "重做上一次更改"),
    ("Fast focus a variable", "快速定位到某个变量"),
    ("Add marker at current cursor", "在当前光标处添加标记"),
    ("Add numbered marker", "添加编号标记"),
    ("Center view at numbered marker", "以编号标记为中心居中显示"),
    ("Add divider", "添加分隔线"),
    ("Go to start", "跳转到起始位置"),
    ("Go to end", "跳转到结束位置"),
    ("Reload waveform", "重新加载波形"),
    ("Go one page/screen right", "向右翻一页/一屏"),
    ("Go one page/screen left", "向左翻一页/一屏"),
    (
        "Go to next transition of focused variable (changeable in config)",
        "跳转到焦点变量的下一次变化（可在配置中修改）",
    ),
    (
        "Go to previous transition of focused variable (changeable in config)",
        "跳转到焦点变量的上一次变化（可在配置中修改）",
    ),
    (
        "Go to next non-zero transition of focused variable",
        "跳转到焦点变量的下一次非零变化",
    ),
    (
        "Go to previous non-zero transition of focused variable",
        "跳转到焦点变量的上一次非零变化",
    ),
    ("Delete focused item", "删除焦点项"),
    ("Toggle full screen", "切换全屏"),
    // help.rs - 启动界面简短控制列表
    ("Scroll down/up", "向下/向上滚动"),
    ("Move focus down/up", "焦点下移/上移"),
    ("Move focused item down/up", "焦点项下移/上移"),
    (
        "Hint: You can repeat keybinds by typing Alt+0-9 before them. For example, Alt+1 Alt+0 k scrolls 10 steps up.",
        "提示：可以在快捷键前输入 Alt+0-9 来重复该操作。例如 Alt+1 Alt+0 k 会向上滚动 10 步。",
    ),
    // help.rs - 许可证窗口
    ("wavAnaly License", "wavAnaly 许可证"),
    ("Dependency licenses", "依赖项许可证"),
    // dialog.rs
    ("State file detected", "检测到状态文件"),
    (
        "A state file was detected in the same directory as the loaded file.\nLoad state?",
        "在已加载文件所在目录中检测到一个状态文件。\n是否加载该状态？",
    ),
    ("Remember my decision for this session", "在本次会话中记住我的选择"),
    ("Load", "加载"),
    ("Don't load", "不加载"),
    ("File Change", "文件已变更"),
    ("File on disk has changed. Reload?", "磁盘上的文件已发生变化，是否重新加载？"),
    ("Reload", "重新加载"),
    ("Leave", "保持不变"),
    // logs.rs
    ("Logs", "日志"),
    ("Error", "错误"),
    ("Warn", "警告"),
    ("Info", "信息"),
    ("Debug", "调试"),
    ("Trace", "追踪"),
    ("Level", "级别"),
    ("Source", "来源"),
    ("Message", "消息"),
    // file_dialog.rs
    ("Open waveform file", "打开波形文件"),
    ("Open command file", "打开命令文件"),
    ("Open Python translator file", "打开 Python 转换脚本文件"),
];
