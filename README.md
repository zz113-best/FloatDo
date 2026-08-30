# FloatDo 🐱

**桌面悬浮 Todo + 个性化 AI 桌宠 + 专注助手** —— 一个完全本地优先的 Windows 桌面效率应用。

> 任务数据、照片、AI Key 全部只存在你自己的电脑上，不上传任何服务器。

## ✨ 功能总览

### 悬浮 Todo

- 无边框、置顶、可拖动的悬浮窗，常驻屏幕右下角
- 收缩态常驻显示**紧急任务** + **最近截止/最早创建**的一条待办，进度一目了然
- 悬停展开完整列表：勾选完成、编辑、拖拽排序、快捷日期、自绘日期时间选择器
- 优先级四色区分（紧急/高/中/低），逾期红色高亮
- 全局快捷键（可自定义）：呼出悬浮窗、快速添加、开始/停止专注
- 超过 8 条自动收起，一键跳转主面板任务页

### 桌宠中心 🐾

- **AI 人像分割抠图**：上传生活照，自动抠出人物（u2netp 模型本地推理，首次使用自动下载约 4.5MB，之后完全离线）；也提供快速几何抠图备选
- **5 种视觉风格**：原图 / Q版贴纸 / 二次元 / 像素风 / 手绘风，本机实时处理
- **多帧逐帧动画**：添加多张姿势照做成轮播动画；单帧自带**呼吸 + 眨眼**微动画
- **像素级鼠标穿透**：只有点在人物身上才算点桌宠，人物之外直接点到桌面
- 大小（64~192px）/ 透明度可调，位置与参数重启保持
- **4 种人格**（温柔 / 激励 / 毒舌 / 高冷）：决定气泡语气，也同步决定 AI 对话人设
- 与任务联动：完成庆祝、到期提醒、专注播报、点击弹今日概览

### AI 对话 🐱

- 兼容 OpenAI / DeepSeek 等标准 `/chat/completions` 接口，API Key 只存本机 SQLite
- 对话时自动注入**真实本地上下文**（待办、逾期、今日专注），回答贴合现状
- AI 回复同步到桌宠气泡

### 专注 + 统计 📊

- 番茄钟式专注/休息循环，桌宠全程陪伴计时
- 统计页：按日专注/完成图表、专注时长**按任务拆分**、任务总览（含逾期完成）
- 全部任务记录：每页 10 条分页表格，支持关键词搜索、完成/逾期/优先级筛选、截止与完成两组日期范围
- 一键导出 CSV（按当前筛选结果，Excel 打开不乱码）

### 任务页 📋

- 按「逾期 → 今天（未完成/已完成）→ 未来（按天）→ 没有截止时间」智能分块
- 与悬浮窗实时同步

## 🚀 快速开始

### 环境要求

- [Node.js](https://nodejs.org/) ≥ 18
- [Rust](https://rustup.rs/)（stable，MSVC 工具链）
- Windows 10/11 + [WebView2 运行时](https://developer.microsoft.com/microsoft-edge/webview2/)（Win11 一般自带）
- VS 2022 Build Tools（C++ 桌面开发工作负载）

### 运行

```bash
npm install
npm run tauri dev
```

首次编译 Rust 依赖需要几分钟。启动后悬浮窗出现在屏幕右下角，桌宠在它旁边。

### 打包安装包

```bash
npm run tauri build
```

产物在 `src-tauri/target/release/bundle/` 下。

### 首次使用提示

- **AI 抠图**：第一次上传照片时会自动从 GitHub 下载分割模型（约 4.5MB）到应用数据目录，之后离线可用；网络不畅时程序会自动切换镜像源
- **AI 对话**：到「设置 → AI 助手」填写 OpenAI 兼容的接口地址、API Key 和模型名；不配置不影响其他功能

## 🧪 测试

```bash
cd src-tauri
cargo test          # 37 个测试：数据层 / 提醒调度 / 抠图算法 / 提示词等
```

## 🏗️ 技术栈与架构

| 层 | 技术 |
|---|---|
| 桌面框架 | Tauri 2（Rust） |
| 前端 | React 18 + TypeScript + Vite + Tailwind CSS + Zustand |
| 存储 | SQLite（rusqlite，单文件，位于 `%APPDATA%/com.floatdo.app/`） |
| AI 分割 | tract（纯 Rust ONNX 推理）+ u2netp |
| AI 对话 | 用户自配的 OpenAI 兼容接口（reqwest） |

分层约定：

```
src/                        # React 界面（窗口按 label 路由：widget / settings / pet）
  components/               # 各页签与悬浮窗组件
  services/                 # invoke / 事件监听的唯一入口（数据访问层）
  stores/                   # Zustand 状态
  utils/                    # 纯函数（排序、时间、气泡文案）
src-tauri/
  src/commands/             # Tauri command 层（只做参数校验与拼装）
  src/database/             # SQL 只出现在这一层（repo + 迁移）
  src/pet_avatar.rs         # 抠图与风格处理纯算法
  src/pet_segment.rs        # u2netp 人像分割（tract 推理）
  src/pet_hit.rs            # 桌宠像素级鼠标穿透
  src/window_pos.rs         # 窗口位置持久化
  src/reminder.rs           # 到期提醒调度
  src/shortcuts.rs          # 全局快捷键
```

业务逻辑不进 React 组件，SQL 不出 repository 层。

## 📄 License

暂未设置开源协议，保留所有权利。如需引用或二次开发请先开 issue 沟通。
