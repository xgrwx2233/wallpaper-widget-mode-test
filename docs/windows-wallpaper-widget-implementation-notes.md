# Windows 壁纸小挂件实现总结

本文记录本测试项目在 Windows 10、Windows 11 下实现 Tauri 壁纸小挂件 `attached` / `detached` 两种模式的关键做法，以及 Win10 退出后残影、标题栏残影的排查和修复经验。

当前项目验证目标：

- 使用 Tauri v2 开发。
- 同一个 Tauri 窗口、同一个 HWND，在 `attached` 和 `detached` 之间切换。
- `attached`：窗口挂到 Explorer 的桌面壁纸层，位于桌面图标下方。
- `detached`：窗口恢复为普通无边框窗口。
- 两种模式都保持无标题栏、无边框、透明窗口效果。
- `attached` 下只要求支持鼠标左键单击。
- `detached` 下按普通窗口处理，支持单击、拖动移动窗口、右下角缩放。
- 兼容 Windows 10 和 Windows 11。

## 核心结论

1. 空标题不等于没有标题栏。

   `SetWindowTextW(hwnd, "")` 只能清掉标题文字，不能移除原生 non-client 区域。Win10 上残影里标题文字消失但仍有标题栏条带，正说明原生 caption 仍被系统画出来了。

2. 不能只依赖 Tauri 的 `decorations: false`。

   Tauri/tao 在 Windows 无边框窗口中，可能仍依靠 `WM_NCCALCSIZE` 把 non-client 区压掉。窗口被挂到 WorkerW 后，尤其在 Win10 上，点击、激活、样式变化可能导致系统重新绘制 caption/frame。

3. `attached` 模式必须避免窗口激活。

   Win10 的标题栏残影主要发生在 attached 后点击窗口，系统把窗口当成可激活普通窗口处理，随后补绘 non-client frame。修复关键是 attached 下保持 `focusable(false)`、`WS_EX_NOACTIVATE`，并在 `WM_MOUSEACTIVATE` 返回 `MA_NOACTIVATE`。

4. Win10 比 Win11 更容易缓存 WorkerW 子窗口的最后一帧。

   Win11 上直接隐藏/退出通常没有残影。Win10 上如果 native caption 被画到 WorkerW，退出后 Explorer 可能把最后一帧缓存成壁纸层残影。重新运行程序、切换 detached、换壁纸、重启 explorer 都能清掉，说明本质是桌面壁纸层缓存刷新问题。

5. 修复残影的最好方式不是退出后强刷，而是避免 caption 被画出来。

   后处理刷新桌面有帮助，但真正稳定的修复是：attached 期间不产生 non-client 区、不激活窗口、不让系统绘制 caption。

## 窗口创建配置

`src-tauri/tauri.conf.json` 中窗口应从创建开始就是桌面 widget 形态：

```json
{
  "label": "widget",
  "title": "",
  "decorations": false,
  "transparent": true,
  "skipTaskbar": true,
  "visible": false,
  "resizable": false,
  "shadow": false,
  "focusable": false
}
```

注意事项：

- `decorations: false` 是必要条件，但不是 Win10 稳定无标题栏的充分条件。
- `shadow: false` 避免无边框窗口额外阴影或 frame 参与 DWM non-client 渲染。
- 初始 `focusable: false` 与 Seelen-UI 的桌面 widget 做法一致，适合默认 attached 启动。
- detached 模式需要在运行时再恢复 `set_focusable(true)`。

## WorkerW / Progman 挂载方式

相关代码：`src-tauri/src/desktop_layer.rs`

桌面壁纸层通常通过 Explorer 的 `Progman` / `WorkerW` 窗口实现。不同 Windows 版本、Explorer 状态、壁纸设置下，WorkerW 结构可能不完全一致，所以不能只找一个固定 HWND。

当前项目做法：

1. 查找 `Progman`：

   ```rust
   FindWindowA(s!("Progman"), None)
   ```

2. 向 `Progman` 发送 `0x052C` 消息，触发或刷新 WorkerW：

   ```rust
   SendMessageTimeoutA(progman, 0x052C, WPARAM(0xD), LPARAM(0x1), ...);
   SendMessageTimeoutA(progman, 0x052C, WPARAM(0), LPARAM(0), ...);
   ```

3. 枚举顶层窗口，找到含有 `SHELLDLL_DefView` 的窗口。该窗口代表桌面图标视图。

4. 查找图标层后面的 sibling `WorkerW`。

5. 额外收集 `Progman` 的子 `WorkerW` 作为候选。

6. 依次尝试候选 WorkerW：

   ```rust
   SetParent(widget_hwnd, worker_w);
   SetWindowPos(widget_hwnd, HWND_BOTTOM, ..., SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_SHOWWINDOW);
   ```

7. 验证：

   - `GetParent(hwnd) == worker_w`
   - `IsWindowVisible(hwnd) == true`

为什么要多个候选：

- Win11 上常见的是 `SHELLDLL_DefView` 相关 WorkerW。
- Win10 上不同 explorer 状态可能出现在 sibling WorkerW 或 Progman child WorkerW。
- 只依赖一个固定结构，会出现 Win11 可用、Win10 attach 失败，或反过来。

## Attached 模式样式

attached 模式要把同一个 Tauri 窗口改成 WorkerW 的 child window，同时彻底移除 non-client frame。

关键 style：

```rust
style |= WS_CHILD | WS_CLIPSIBLINGS | WS_CLIPCHILDREN;
style &= !WS_POPUP;
style &= !WS_CAPTION;
style &= !WS_THICKFRAME;
style &= !WS_SYSMENU;
style &= !WS_MINIMIZEBOX;
style &= !WS_MAXIMIZEBOX;
style &= !WS_BORDER;
style &= !WS_DLGFRAME;
```

关键 extended style：

```rust
ex_style |= WS_EX_TOOLWINDOW;
ex_style |= WS_EX_NOACTIVATE;
ex_style &= !WS_EX_APPWINDOW;
ex_style &= !WS_EX_WINDOWEDGE;
ex_style &= !WS_EX_CLIENTEDGE;
ex_style &= !WS_EX_DLGMODALFRAME;
```

重点：

- `WS_EX_NOACTIVATE` 是 Win10 去除点击后标题栏残影的关键之一。
- attached 下不要让窗口进入任务栏。
- attached 下不要让窗口变成可激活普通窗口。
- attached 下不要动态恢复 resizable。

Tauri 侧也要同步：

```rust
window.set_focusable(false)?;
window.set_resizable(false)?;
window.set_skip_taskbar(true)?;
```

## Detached 模式样式

detached 模式仍然是同一个 HWND，只是解除 WorkerW parent，恢复普通顶层无边框窗口。

关键操作：

```rust
SetParent(hwnd, None);
style |= WS_POPUP;
style &= !WS_CHILD;
style &= !WS_CAPTION;
style &= !WS_THICKFRAME;
style &= !WS_SYSMENU;
style &= !WS_MINIMIZEBOX;
style &= !WS_MAXIMIZEBOX;
style &= !WS_BORDER;
style &= !WS_DLGFRAME;
```

detached 下要恢复普通窗口交互：

```rust
window.set_focusable(true)?;
window.set_resizable(true)?;
window.set_skip_taskbar(false)?;
window.set_focus()?;
```

注意：

- detached 也要保持无标题栏、无边框，不应该恢复 `WS_CAPTION`。
- detached 的拖动和缩放使用 Tauri 普通 API，例如 `startDragging()`、`startResizeDragging("SouthEast")`。
- 拖动热区应只覆盖 UI 空白区，不能盖住按钮和统计面板。

## Native frame guard

Win10 上仅移除 style 仍可能不够，因为 Tauri/tao 或 Windows 可能在某些消息中重新计算 non-client frame。因此当前项目安装了一个很小的 WndProc guard。

相关函数：

- `install_native_frame_guard`
- `native_frame_guard_wnd_proc`

拦截策略：

1. `WM_NCCALCSIZE`

   返回 `0`，让整个窗口区域都作为 client area，防止系统保留标题栏和边框区域。

2. `WM_NCPAINT`

   返回 `0`，阻止系统绘制 non-client frame。

3. `WM_NCACTIVATE`

   返回 `0`，阻止激活态标题栏绘制。

4. `WM_MOUSEACTIVATE`

   只在 attached 模式返回 `MA_NOACTIVATE`，避免点击后激活窗口。

   detached 模式不能这样拦，否则普通窗口会变得不可正常聚焦。

5. `WM_STYLECHANGING`

   如果系统或 Tauri 试图重新加入 frame 相关样式，直接剥掉：

   ```rust
   styleNew &= !WS_CAPTION;
   styleNew &= !WS_THICKFRAME;
   styleNew &= !WS_SYSMENU;
   styleNew &= !WS_MINIMIZEBOX;
   styleNew &= !WS_MAXIMIZEBOX;
   ```

6. `WM_STYLECHANGED`、`WM_WINDOWPOSCHANGING`、`WM_WINDOWPOSCHANGED`

   做 debug 记录，辅助判断点击后是否发生了样式变化。

## DWM non-client 渲染

当前项目在 attached / detached 切换时调用：

```rust
DwmSetWindowAttribute(
    hwnd,
    DWMWA_NCRENDERING_POLICY,
    DWMNCRP_DISABLED,
    size_of::<i32>(),
);
```

作用：

- 禁止 DWM 按窗口样式渲染 non-client 区。
- 降低 Win10 在 WorkerW 上缓存标题栏 frame 的概率。

注意：

- 这不是单独充分条件，仍需配合 style 清理和 WndProc guard。
- Win11 对这个问题不敏感，但保留该处理不会破坏 Win11。

## Attached 下鼠标交互

attached 窗口在桌面图标下方，不应依赖普通 hover 交互。当前项目只要求 left click。

做法：

- 后端线程读取：
  - `GetCursorPos`
  - `GetWindowRect`
  - `GetAsyncKeyState(VK_LBUTTON)`
- 当鼠标位于窗口矩形内，并检测到左键从 up 到 down，向前端发送 `desktop-input` 事件。

相关代码：`src-tauri/src/input_forwarder.rs`

注意：

- attached 模式不支持 hover 是合理的。
- attached click 不应让窗口激活。
- 如果需要复杂交互，可以考虑独立透明 interaction proxy，但 proxy 窗口本身也必须无标题栏、无边框、不可激活，否则会再次引入标题栏残影风险。

## Win10 残影问题复盘

现象：

- Win11：attached 退出正常，没有残影。
- Win10：attached 退出后壁纸上留下不可点击、不可交互的残影。
- 先后出现过完整窗口残影、只剩标题栏残影、标题栏无文字但条带仍在。
- detached 模式退出没有残影。
- attached 切回 detached 后，原位置没有残影。
- 重新运行程序、切换壁纸、重启 Explorer 后残影消失。

判断：

- 不是前端 DOM 标题栏。
- 不是新建了第二个代理窗口。
- 是同一个 HWND 在 attached 后被 Win10 绘制了 native non-client/caption。
- 空标题只清除了标题文本，没有移除 caption。
- Win10 的 WorkerW / 桌面壁纸层缓存了最后一帧。

最终有效修复组合：

1. attached 下 `set_focusable(false)`。
2. attached style 带 `WS_EX_NOACTIVATE`。
3. attached 点击时 `WM_MOUSEACTIVATE -> MA_NOACTIVATE`。
4. `WM_NCCALCSIZE -> 0`，保持 client-only。
5. `WM_NCPAINT`、`WM_NCACTIVATE` 返回 `0`，阻止系统画 frame。
6. `WM_STYLECHANGING` 剥离 `WS_CAPTION`、`WS_THICKFRAME`、`WS_SYSMENU` 等。
7. DWM non-client rendering policy 设为 disabled。
8. 关闭时先 detach / hide / destroy，再刷新 WorkerW / Progman / 当前壁纸。

## 退出清理

Win10 下退出 attached 窗口前需要认真清理。

当前流程包括：

1. 前端点击 Close。
2. 后端 `prepare_close_app`：
   - 状态改为 detached。
   - `detach_from_desktop_icon_layer`。
   - 恢复 focusable / resizable / taskbar。
   - 通知前端 `close-prepared`。
3. 延迟后 `finish_close_app`：
   - 设置允许退出。
   - `cleanup_desktop_layer_before_exit`。
   - hide window。
   - `app.exit(0)`。

cleanup 中做了：

- `WM_SETREDRAW(false)` 停止新绘制。
- 确保窗口仍是 borderless top-level 样式。
- `ShowWindow(SW_HIDE)` 和 `SWP_HIDEWINDOW`。
- `DwmFlush` / `GdiFlush`。
- 刷新旧 parent、Progman、WorkerW 候选。
- `DestroyWindow(hwnd)`。
- `PaintDesktop(old_parent)`。
- 再次刷新 WorkerW / Progman。
- 重新触发当前壁纸设置，促使 Explorer 刷新壁纸层。

经验：

- 只在退出后强刷桌面，不如先确保 attached 期间没有 native frame。
- 如果 Win10 仍出现残影，优先看是否点击后产生了 `WS_CAPTION` 或是否丢了 `WS_EX_NOACTIVATE`。

## 调试字段

界面上 `Diag` 和 debug log 显示以下关键字段：

- `Style`：窗口 `GWL_STYLE`。
- `Ex`：窗口 `GWL_EXSTYLE`。
- `ParentOK`：是否挂到候选 WorkerW。
- `FG`：当前窗口是否 foreground。
- `Hook`：native frame guard 是否安装。
- `Native`：最近一次 native frame 相关消息。
- `Count`：native frame guard 记录的消息计数。

常用样式位：

- `WS_CAPTION = 0x00C00000`
- `WS_THICKFRAME = 0x00040000`
- `WS_CHILD = 0x40000000`
- `WS_POPUP = 0x80000000`
- `WS_EX_NOACTIVATE = 0x08000000`

attached 下重点检查：

- `Style` 不应包含 `WS_CAPTION`、`WS_THICKFRAME`。
- `Style` 应包含 `WS_CHILD`。
- `Ex` 应包含 `WS_EX_NOACTIVATE`。
- 点击后 `FG` 不应变成 `Y`。
- 点击后不应出现不透明标题栏。

detached 下重点检查：

- `Style` 应包含 `WS_POPUP`，不应包含 `WS_CHILD`。
- 不应包含 `WS_CAPTION`。
- `Ex` 不应包含 `WS_EX_NOACTIVATE`，否则普通窗口可能无法正常聚焦。

## Win10 / Win11 兼容注意事项

Win10：

- WorkerW / Explorer 缓存更敏感。
- 点击 attached 窗口后更容易出现 native caption。
- 退出后更容易留下最后一帧残影。
- 必须避免窗口激活和 non-client 绘制。
- 必须准备多候选 WorkerW 查找。

Win11：

- WorkerW attach 通常更宽容。
- non-client 残影问题不明显。
- 仍建议使用同一套严格样式，避免不同版本 Explorer 行为差异。

共同注意：

- 不要把 attached / detached 做成两个不同窗口对象，除非有明确理由。
- 如果必须使用代理窗口，代理窗口也必须遵守无标题栏、无边框、不可激活规则。
- 不要用清空标题当作移除标题栏。
- 不要在 attached 模式恢复 `set_focusable(true)`。
- 不要让 attached 窗口进入任务栏。
- 不要依赖单一 WorkerW。
- 样式变更后调用 `SetWindowPos(..., SWP_FRAMECHANGED | SWP_NOACTIVATE)`。

## 推荐测试清单

每次修改 attach / detach 逻辑后，在 Win10 和 Win11 都执行：

1. 启动程序，确认默认 attached。
2. 桌面图标应在小挂件上方，小挂件在壁纸层。
3. attached 下左键单击小挂件：
   - click 计数增加。
   - 不出现标题栏。
   - UI 元素不上移。
   - `FG` 不变成 `Y`。
4. 点击 Detach：
   - 变成普通无边框窗口。
   - 可拖动。
   - 可右下角缩放。
   - 可正常单击。
5. 点击 Attach：
   - 回到壁纸层。
   - 再次点击不出现标题栏。
6. attached 下退出：
   - Win10、Win11 都不留残影。
7. detached 下退出：
   - 不留残影。
8. 如果有残影，换壁纸能否清除：
   - 能清除，说明仍是 WorkerW / 壁纸层缓存问题。
   - 回看是否产生 native caption 或是否丢失 `NOACTIVATE`。

## 当前项目文件对应关系

- `src-tauri/src/desktop_layer.rs`
  - WorkerW 查找。
  - attach / detach。
  - native style 处理。
  - WndProc frame guard。
  - 退出清理和桌面刷新。

- `src-tauri/src/input_forwarder.rs`
  - attached 模式下左键单击转发。

- `src-tauri/src/lib.rs`
  - Tauri command。
  - attached / detached 状态切换。
  - `focusable`、`resizable`、`skip_taskbar` 运行时切换。
  - 启动默认 attach。

- `src-tauri/tauri.conf.json`
  - 初始无边框、透明、不可聚焦窗口配置。

- `src/main.tsx`
  - 模式切换 UI。
  - debug snapshot 展示。
  - detached 普通 DOM click / drag / resize。

- `src/styles.css`
  - 透明小挂件视觉。
  - detached 拖动热区。
  - 诊断面板布局。

## 最小实现顺序建议

如果在新项目里复用，建议按这个顺序实现：

1. 先创建 `decorations(false) + transparent(true) + shadow(false) + focusable(false)` 的 Tauri 窗口。
2. 实现 WorkerW 多候选查找。
3. 实现 `SetParent(hwnd, worker_w)`。
4. attached 样式加入 `WS_CHILD`，清掉所有 caption/frame 样式。
5. attached ex style 加 `WS_EX_NOACTIVATE`。
6. detached 只解除 parent，不换新窗口对象。
7. 加 WndProc guard，拦 `WM_NCCALCSIZE`、`WM_NCPAINT`、`WM_NCACTIVATE`、`WM_MOUSEACTIVATE`、`WM_STYLECHANGING`。
8. 加 debug 面板，输出 style/exstyle/parent/foreground/native message。
9. 最后再做退出清理和桌面刷新。

这样可以避免先做复杂 UI 后再反查 Win10 原生窗口问题。
