# radiobar

Tauri 2 + TypeScript（无前端框架）的极简桌面电台播放器，目标是一个 340×64 的无边框常驻小窗。

## 环境要求

- Windows 10/11（x64）
- Rust stable（MSVC 工具链）
- Node.js 20.19+ / 22.12+
- Visual Studio 2022 Build Tools + Windows SDK

## 开发

```powershell
npm install
npm run tauri dev
```

Vite 开发服务器固定在 `http://localhost:1420`。

## 构建

```powershell
npm run build          # 前端（Vite）
npm run tauri build    # 安装包（NSIS）
```

## 快速检查

```powershell
npm run test           # Vitest
cargo check            # 在 src-tauri/ 下
```

## 目录

- `src/` — TypeScript 前端入口与样式
- `src-tauri/` — Rust 后端（窗口、托盘、播放逻辑在后续任务加入）
- `docs/superpowers/` — 设计文档与实现计划
