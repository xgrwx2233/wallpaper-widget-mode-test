我现状有一个难题请帮我分析解决：1、我目前的项目主要是在win11上开发的，我的课程表小挂件要挂在壁纸层上桌面下，因此比较底层。我在win11上运行正常。但是到了win10上却不正常。现在我在win10上找到了解决方案，但是需要改很多代码。2、并且我现在要把项目升级，添加一个独立的聊天通信功能模块。因此我想能不能把项目的小挂件模块独立出来，在最后打包成exe时，课程表小挂件分别用win10/win11的代码打包，聊天通信模块共同打包。在用户拿到exe文件进行安装时，自动检测当前系统，将小挂件部分安装对应系统的版本，聊天通信模块安装共同的版本。或者将小挂件内部的通用部分也提取出来，做为共同安装部分，只把系统特定部分分离出来，根据系统安装对应版本。

可以，而且你这个想法是**正确方向**。但我建议你不要把问题理解成“安装时只安装 Win10 或 Win11 某一个版本”，而是应该设计成：

> **一个教师助手主程序 + 一个通用聊天通信模块 + 两套系统专用的小挂件底层适配模块。**

你的项目文档里本来也已经明确：**只有课程表小挂件需要 attach 到壁纸/桌面层，聊天面板、登录、设置、好友资料、群资料等窗口都应该是普通窗口**，所以聊天通信模块不应该被 Win10/Win11 桌面层兼容问题拖进去。

---

# 一、先给结论

你可以做成这样：

```text
TeacherAssistant.exe              教师助手主程序，通用
├── 聊天通信模块                    通用，Win10 / Win11 共用
├── 登录 / 设置 / 好友 / 群聊窗口      通用，Win10 / Win11 共用
├── 课程表数据 / 同步 / API           通用，Win10 / Win11 共用
├── widget-host-win10.exe          Win10 专用小挂件宿主
└── widget-host-win11.exe          Win11 专用小挂件宿主
```

安装包里可以同时带上：

```text
TeacherAssistant.exe
widget-host-win10.exe
widget-host-win11.exe
```

用户安装时不用手动选择。程序第一次启动时自动检测系统，然后启动对应的小挂件宿主：

```text
Win10 → 启动 widget-host-win10.exe
Win11 → 启动 widget-host-win11.exe
```

这比“安装时只安装某一个版本”更稳。

---

# 二、我不建议你在安装阶段只装一个小挂件版本

你提出的方案是：

```text
安装时检测系统
Win10 就安装 Win10 小挂件代码
Win11 就安装 Win11 小挂件代码
聊天模块共同安装
```

这个可以做，但我不建议作为第一优先方案。

原因是：

```text
1. 安装器逻辑会变复杂
2. 用户 Win10 以后升级到 Win11，原来的安装内容可能不匹配
3. 系统检测如果出错，用户无法自救
4. 后续自动更新时还要区分 Win10 / Win11 更新包
5. Codex 修改安装器脚本，出错概率比修改业务模块更高
```

更推荐：

```text
安装包同时安装 Win10 / Win11 两个小挂件宿主
主程序运行时选择启动哪个
```

这样即使用户系统升级，也可以下一次启动时重新判断。

---

# 三、最推荐的架构：主程序 + 小挂件 Sidecar

Tauri 本身支持把外部二进制程序作为 sidecar 一起打包，官方文档说明可以在 `tauri.conf.json` 的 `bundle.externalBin` 里加入外部二进制文件；Windows 端也可以通过 Tauri CLI 构建 NSIS/MSI 安装器。([Tauri][1]) ([Tauri][2])

你可以把项目拆成：

```text
apps/
  desktop-main/                 教师助手主程序
    ├── 聊天
    ├── 联系人
    ├── 群聊
    ├── 对讲入口
    ├── 设置
    └── 小挂件启动器

  widget-host-win10/            Win10 小挂件宿主
    └── 只处理 Win10 桌面层 attach

  widget-host-win11/            Win11 小挂件宿主
    └── 只处理 Win11 桌面层 attach

packages/
  schedule-core/                课程表通用逻辑
  schedule-ui/                  课程表 UI 组件
  desktop-ipc-protocol/         主程序和小挂件通信协议
  api-client/                   后端接口
  shared-types/                 共享类型
```

也可以不拆成多个仓库，而是在一个 monorepo 里拆多个 app。你之前的规划里也建议过类似结构：`apps/desktop-tauri`、`server`、`packages/protocol`、`generated-typescript-client` 等，核心思想就是共享协议和通用代码，不让不同端各自乱写一套。

---

# 四、哪些代码应该拆出去？

你的代码不要按“Win10 一份完整项目、Win11 一份完整项目”复制。应该拆成三层。

## 1. 通用层

这些 Win10 / Win11 都一样：

```text
课程表数据结构
课程表渲染 UI
课程表同步
登录状态
用户信息
设置项
主题 / 外观
右键菜单业务逻辑
后端 API 调用
本地缓存
聊天入口
```

例如：

```text
schedule-core/
  Course.ts
  Timetable.ts
  ScheduleService.ts
  ScheduleRepository.ts

schedule-ui/
  CourseCard.tsx
  ScheduleWidgetView.tsx
  WidgetMenu.tsx
```

## 2. 系统适配层

只有这些需要分 Win10 / Win11：

```text
查找桌面 WorkerW / Progman / SHELLDLL_DefView
把窗口挂到桌面层
设置窗口层级
处理 Explorer 重启
处理多显示器
处理透明 / 点击穿透
处理 DPI / 缩放
处理窗口恢复
```

建议抽象成接口：

```ts
interface DesktopLayerAdapter {
  attachWidgetWindow(windowLabel: string): Promise<void>
  detachWidgetWindow(windowLabel: string): Promise<void>
  recoverAfterExplorerRestart(): Promise<void>
  getAttachStatus(): Promise<AttachStatus>
}
```

Rust 侧也可以类似：

```rust
trait DesktopLayerAdapter {
    fn attach(&self, hwnd: isize) -> Result<()>;
    fn detach(&self, hwnd: isize) -> Result<()>;
    fn recover_after_explorer_restart(&self) -> Result<()>;
}
```

然后分别实现：

```text
Win10DesktopLayerAdapter
Win11DesktopLayerAdapter
```

## 3. 聊天通信层

聊天通信模块不要依赖桌面挂载逻辑。

你项目计划里聊天窗口本来就应该是普通桌面窗口，不参与 wallpaper attach / proxy / desktop layer 逻辑。

所以聊天模块应该这样：

```text
ChatWindowHost
ChatWindow
ConversationList
MessageTimeline
RtcService
WebSocketClient
ChatRepository
```

它只关心：

```text
HTTP API
WebSocket
RTC
本地缓存
通知
窗口打开关闭
```

不关心：

```text
桌面壁纸层
WorkerW
Win10 / Win11 attach 差异
```

---

# 五、主程序如何自动选择 Win10 / Win11 小挂件？

建议不要在安装器里判断，而是在主程序启动时判断。

流程：

```text
TeacherAssistant.exe 启动
  ↓
检测 Windows 版本 / 桌面层能力
  ↓
判断应该使用 Win10 适配器还是 Win11 适配器
  ↓
启动 widget-host-win10.exe 或 widget-host-win11.exe
  ↓
主程序通过 IPC 和小挂件通信
```

Windows 版本检测时要注意：微软文档里 Windows 10 和 Windows 11 的主版本号都是 10.0，不能只看 major/minor；并且微软也提醒，单纯依赖版本信息不一定是测试功能可用性的最佳方式，最好根据你真正需要的功能做能力检测。([Microsoft Learn][3])

实际工程里可以做两层判断：

```text
第一层：系统版本判断
Windows build >= 22000 → 倾向 Win11
Windows build < 22000  → 倾向 Win10

第二层：桌面 attach 能力探测
尝试当前适配器 attach
失败则切换备用适配器
```

这样更稳。

---

# 六、主程序和小挂件如何通信？

如果小挂件做成独立 sidecar 进程，主程序和小挂件之间需要通信。

推荐三种方式，从简单到复杂：

## 方案 1：本地 HTTP / WebSocket

小挂件启动一个本地端口：

```text
127.0.0.1:32145
```

主程序发送：

```text
/update-schedule
/show-widget
/hide-widget
/reload-theme
```

优点：

```text
开发简单
前后端都容易调
Codex 容易理解
```

缺点：

```text
要处理端口占用
要做本地鉴权 token
```

## 方案 2：Named Pipe

Windows 本地进程通信可以用 Named Pipe。

优点：

```text
更像本地桌面软件
安全性更好
不占 TCP 端口
```

缺点：

```text
实现比 HTTP/WebSocket 稍复杂
```

## 方案 3：共享本地数据库 + 事件通知

课程表数据写本地 SQLite，小挂件读取。

不建议第一版这样做，因为两个进程同时读写 SQLite 容易出现锁和同步问题。更好的方式是：

```text
主程序负责数据
小挂件负责展示
主程序主动把数据推给小挂件
```

---

# 七、两种可选落地方案

## 方案 A：一个 Tauri 程序，内部根据 Win10 / Win11 选择 adapter

结构：

```text
TeacherAssistant.exe
  ├── ChatWindow
  ├── ScheduleWidgetWindow
  ├── Win10DesktopLayerAdapter
  └── Win11DesktopLayerAdapter
```

优点：

```text
进程少
打包简单
状态共享简单
本地缓存简单
```

缺点：

```text
Win10 / Win11 底层代码在同一个 Rust/Tauri 项目里
如果两套实现差异很大，容易互相影响
一次崩溃可能影响整个主程序
```

适合：

```text
Win10 和 Win11 差异集中在少量 Rust 文件里
只是 attach 逻辑不同
UI 和窗口结构基本一致
```

## 方案 B：主程序 + 两个小挂件 sidecar

结构：

```text
TeacherAssistant.exe
widget-host-win10.exe
widget-host-win11.exe
```

优点：

```text
Win10 / Win11 底层代码彻底隔离
聊天模块不受影响
小挂件崩了不一定影响聊天主程序
后续可单独排查 Win10 问题
可以灰度切换小挂件宿主
```

缺点：

```text
打包复杂一点
需要 IPC
需要管理多个进程
自动更新时要更新多个 exe
```

适合你现在的情况，因为你说：

```text
Win10 解决方案需要改很多代码
项目又要新增独立聊天通信模块
```

所以我更建议你用 **方案 B**。

---

# 八、安装包怎么做？

推荐安装包里同时包含：

```text
TeacherAssistant.exe
widget-host-win10.exe
widget-host-win11.exe
```

安装目录例如：

```text
C:\Program Files\TeacherAssistant\
  TeacherAssistant.exe
  sidecars\
    widget-host-win10.exe
    widget-host-win11.exe
  resources\
  config\
```

第一次启动：

```text
TeacherAssistant.exe 检测系统
  ↓
选择 sidecars/widget-host-win10.exe 或 sidecars/widget-host-win11.exe
  ↓
写入本地配置
```

本地配置例如：

```json
{
  "widget_engine": "win10",
  "auto_detected": true,
  "last_attach_success": true
}
```

如果检测错了，可以在设置里留一个高级选项：

```text
小挂件兼容模式：
- 自动
- Win10 模式
- Win11 模式
```

这个非常重要。因为桌面层 attach 这种底层功能很容易受系统版本、Explorer 状态、多显示器、第三方桌面软件影响。保留手动切换可以大幅减少售后问题。

---

# 九、为什么聊天模块应该放主程序，而不是放小挂件？

因为聊天是普通窗口，不需要挂桌面层。

你的项目计划里聊天窗口验收标准也明确要求：聊天窗口不是壁纸小挂件，不会被 attach 到桌面图标层，并且有自建标题栏、支持关闭、最小化、最大化。

所以应该这样：

```text
TeacherAssistant.exe
  负责：
    登录
    设置
    聊天
    群聊
    对讲
    联系人
    云同步
    打开/关闭小挂件
    管理 sidecar

widget-host-win10/win11.exe
  只负责：
    课程表小挂件窗口
    桌面层 attach
    小挂件渲染
```

不要让小挂件模块承担聊天通信。否则 Win10 问题会继续拖累整个项目。

---

# 十、推荐你的最终目录结构

```text
teacher-assistant/
  apps/
    desktop-main/
      src/
        app/
          MainWindowHost.tsx
          ChatWindowHost.tsx
          SettingsWindowHost.tsx
        features/
          chat/
          contact/
          rtc/
          schedule/
          widget-launcher/
        src-tauri/
          src/
            main.rs
            commands/
              open_chat_window.rs
              launch_widget.rs
              detect_windows.rs

    widget-host-win10/
      src/
        WidgetHost.tsx
      src-tauri/
        src/
          main.rs
          desktop_layer_win10.rs

    widget-host-win11/
      src/
        WidgetHost.tsx
      src-tauri/
        src/
          main.rs
          desktop_layer_win11.rs

  packages/
    schedule-core/
    schedule-ui/
    api-client/
    websocket-client/
    rtc-core/
    shared-types/
    widget-ipc-protocol/

  server/
    api/
    websocket/
    rtc-token/
    file/
    sync/

  docs/
    desktop-architecture.md
    widget-adapter.md
    windows-compatibility.md
```

---

# 十一、开发顺序建议

你现在不要马上大拆项目。建议分 5 步。

## 第 1 步：先定义边界

先写一个文档：

```text
docs/desktop-module-boundary.md
```

明确：

```text
聊天模块永远是普通窗口
课程表小挂件才允许 attach 到桌面层
Win10 / Win11 差异只允许存在于 widget host 或 desktop layer adapter
```

## 第 2 步：抽出通用课程表 UI 和数据

先把这些抽出去：

```text
课程表数据模型
课程表渲染组件
课程表设置
课程表 API
```

不要动 Win10 / Win11 底层代码。

## 第 3 步：把桌面 attach 逻辑封装成 adapter

先不拆 exe，只做接口封装：

```text
DesktopLayerAdapter
Win11DesktopLayerAdapter
Win10DesktopLayerAdapter
```

这一步能让代码从“到处都是底层代码”变成“只有一个入口”。

## 第 4 步：再决定是否 sidecar 化

如果封装后发现差异不大，就保留一个 Tauri 程序内部选择 adapter。

如果封装后发现 Win10 还是改动很大，就拆成：

```text
widget-host-win10.exe
widget-host-win11.exe
```

## 第 5 步：最后再处理安装包

先别让 Codex 一开始改安装器。安装器是最后一步。

---

# 十二、给 Codex 的任务拆法

你可以这样发给 Codex，避免它乱改。

## 任务 1：分析边界，不改代码

```text
请分析当前 Tauri Windows 项目中课程表小挂件和普通窗口的边界。

要求：
1. 找出所有与 wallpaper attach、WorkerW、Progman、desktop layer、SetParent、窗口层级相关的代码。
2. 找出聊天窗口、设置窗口、登录窗口等普通窗口代码。
3. 输出模块边界建议。
4. 本次不修改任何代码。
5. 明确哪些代码必须保留在小挂件模块，哪些代码可以进入通用模块。
```

## 任务 2：封装 DesktopLayerAdapter

```text
请把当前课程表小挂件的桌面层挂载逻辑封装成 DesktopLayerAdapter。

要求：
1. 新增 DesktopLayerAdapter 接口。
2. 当前 Win11 可用逻辑移动到 Win11DesktopLayerAdapter。
3. 不改变现有运行行为。
4. 聊天窗口、设置窗口、登录窗口不得引用 DesktopLayerAdapter。
5. 保证 npm run build 和 Rust check 通过。
```

## 任务 3：加入 Win10 adapter

```text
请新增 Win10DesktopLayerAdapter，实现 Win10 上课程表小挂件 attach 到桌面层的逻辑。

要求：
1. 不修改聊天模块。
2. 不修改普通窗口创建逻辑。
3. 根据系统检测选择 Win10 或 Win11 adapter。
4. 增加日志：当前系统版本、选择的 adapter、attach 结果。
5. attach 失败时不要导致主程序崩溃。
```

## 任务 4：评估 sidecar 拆分

```text
请评估是否需要把课程表小挂件拆成独立 sidecar 进程。

要求：
1. 分析当前小挂件与主程序的数据依赖。
2. 给出主程序与 widget-host 的 IPC 协议。
3. 给出 sidecar 目录结构。
4. 不实际拆分代码。
```

## 任务 5：真正拆分 sidecar

```text
请将课程表小挂件拆分为独立 widget-host sidecar。

要求：
1. desktop-main 保留聊天、登录、设置、联系人、对讲入口。
2. widget-host 只负责课程表小挂件窗口和桌面层 attach。
3. 主程序负责检测系统并启动对应 widget-host。
4. 主程序和 widget-host 通过本地 IPC 同步课程表数据。
5. 安装包同时包含 widget-host-win10 和 widget-host-win11。
```

---

# 十三、我给你的最终建议

你的情况最适合：

```text
主程序通用化
聊天模块独立普通窗口化
课程表小挂件宿主拆分
Win10 / Win11 桌面层逻辑隔离
安装包同时带两个 widget host
运行时自动选择
```

一句话：

> **不要让 Win10/Win11 桌面层兼容问题污染聊天通信模块。把“课程表小挂件底层宿主”独立出来，聊天通信作为主程序通用模块。**

我建议你的最终形态是：

```text
TeacherAssistant.exe
  = 登录 + 设置 + 聊天 + 群聊 + 对讲 + 同步 + 小挂件管理

widget-host-win10.exe
  = Win10 桌面层小挂件

widget-host-win11.exe
  = Win11 桌面层小挂件
```

这样后面你开发 RTC、聊天、Redis、OSS、MySQL、WebSocket 时，都不会被 Win10 壁纸层问题反复干扰。

[1]: https://v2.tauri.app/develop/sidecar/?utm_source=chatgpt.com "Embedding External Binaries"
[2]: https://v2.tauri.app/distribute/windows-installer/?utm_source=chatgpt.com "Windows Installer"
[3]: https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/ns-wdm-_osversioninfoexw?utm_source=chatgpt.com "OSVERSIONINFOEXW (wdm.h) - Windows drivers"
