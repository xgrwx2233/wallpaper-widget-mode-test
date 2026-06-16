# Wallpaper Widget Mode Test

Tauri v2 Windows 测试项目，用于验证壁纸小挂件的两种模式：

- `attached`：窗口挂到 Explorer `WorkerW`，显示在壁纸上、桌面图标下。
- `detached`：解除挂载，恢复为普通无边框窗口，支持拖动、缩放和正常 DOM 鼠标事件。

## 运行

```powershell
npm install
npm run tauri:dev
```

也可以直接运行已构建 exe：

```powershell
.\src-tauri\target\release\wallpaper-widget-mode-test.exe
```

## 构建

```powershell
npm run build
cd src-tauri
cargo check
cd ..
npm run tauri:build
```

构建产物：

- `src-tauri/target/release/wallpaper-widget-mode-test.exe`
- `src-tauri/target/release/bundle/msi/Wallpaper Widget Mode Test_0.1.0_x64_en-US.msi`
- `src-tauri/target/release/bundle/nsis/Wallpaper Widget Mode Test_0.1.0_x64-setup.exe`

## 交互验证

- 默认尝试 attached 启动；如果当前 Explorer/WorkerW 状态不可用，会兜底为 detached，不会直接退出。
- Windows 11 下已兼容两种 Explorer 桌面层：标准 `WorkerW` 兄弟窗口，以及 `Progman` 下的 Raised Desktop `WorkerW`。
- 界面中的 `Diag` 按钮会刷新诊断行：`StdWorker`、`ProgWorker`、`ParentOK`、`Visible` 可用于判断 attached 失败位置。
- 在 attached 模式下，单击面板会通过 Rust `desktop-input` 转发计数。
- 单击 `Detach` 切到普通窗口。
- detached 模式下可拖动顶部区域、拖右下角缩放，单击面板由 DOM click 计数。
- 单击 `Attach` 可重新挂回桌面壁纸层。
