# Codex Switch

Codex Switch 是一个 Windows 桌面端 Codex 账号切换工具，用于在本机管理多个 Codex OAuth 登录凭据，并在需要时切换当前 Windows 用户下的 Codex 登录状态。

项目基于 Tauri 2、Vue 3、TypeScript 和 Yarn 构建。

## 功能

- 导入单个或多个 Codex OAuth 凭据文件。
- 在本地账号库中保存多个 Codex 账号。
- 切换账号时替换 `auth.json`，并按“当前配置优先”合并 `config.toml`。
- 尽量保留当前本机已有的 Codex 工具、插件、MCP servers、projects 等配置。
- 切换前自动备份当前 Codex 登录状态。
- 支持从备份中恢复 `auth.json` 和 `config.toml`。
- 支持通过 OAuth 登录保存新账号。
- 每个 OAuth 登录账号使用独立 WebView2/Profile 目录，隔离 cookie、cache、localStorage。
- 支持手动刷新 token。
- 支持手动检查 Codex 额度/状态，额度接口为 best effort。
- 支持启动时检查更新，并通过 GitHub Release / Tauri updater 发布安装包。
- 使用紧凑暗色桌面 UI 和自定义标题栏。

## 默认路径

Codex Switch 默认操作当前 Windows 用户的 Codex home：

```text
C:\Users\Y\.codex
```

这里通常包含：

```text
C:\Users\Y\.codex\auth.json
C:\Users\Y\.codex\config.toml
```

软件自己的数据默认保存在：

```text
C:\Users\Y\AppData\Roaming\codex-account-switcher
```

常见目录：

```text
settings.json       软件设置
accounts\           导入或 OAuth 登录保存的账号库
backups\            切换账号或手动备份产生的备份
browser-profiles\   OAuth 登录使用的隔离 WebView2/Profile 数据
```

## Codex OAuth 文件说明

Codex OAuth JSON 是本地登录凭据，不是普通 OpenAI API Key。

在当前 Codex 桌面端和 Codex CLI 的使用方式中，它们可能共享同一个 Codex home，例如：

```text
C:\Users\Y\.codex
```

因此替换该目录下的 `auth.json` 可能会同时影响同一 Windows 用户下 Codex 桌面端和 Codex CLI 的登录状态。

## 安全提醒

Codex Switch 会在本地保存导入或登录得到的 OAuth 凭据，这些文件可能包含 `refresh_token`。

请不要分享以下目录或文件：

- 软件数据目录
- `accounts` 账号库
- `backups` 备份目录
- 导出的 OAuth JSON 文件
- 当前 Codex home 下的 `auth.json`

如果这些文件泄露，可能导致你的账号登录凭据被他人使用。

## 免责声明

本项目仅用于学习、研究和个人本地账号管理场景，不是 OpenAI、Codex 或相关服务的官方产品，也不与其存在任何官方关联。

使用本软件所产生的一切后果由使用者自行承担，包括但不限于：

- 账号登录状态异常
- 账号切换失败或配置丢失
- 凭据、token、配置文件或备份文件泄露
- Codex CLI / Codex 桌面端状态被影响
- 额度查询、token 刷新、OAuth 登录失败
- 第三方服务策略、风控、地区限制或账号规则导致的异常
- 因使用、误用、传播或修改本软件造成的任何直接或间接损失

作者不对任何账号封禁、订阅异常、额度消耗、数据丢失、凭据泄露或其他损失承担责任。

请仅用于你自己拥有或有权使用的账号，并遵守相关服务条款、法律法规和平台规则。

## 问题反馈

如果你在使用过程中遇到 bug，或有功能建议，欢迎通过以下方式反馈：

- 在 GitHub 仓库提交 Issues
- 发送邮件到：<qianmang1@gmail.com>

## 参考项目

本项目在 UI 设计、额度展示思路、OAuth/配置处理等方面参考和学习了以下开源项目或本地代码摘录：

- [router-for-me/Cli-Proxy-API-Management-Center](https://github.com/router-for-me/Cli-Proxy-API-Management-Center)
- [router-for-me/CLIProxyAPI](https://github.com/router-for-me/CLIProxyAPI)

感谢相关项目的开源工作。本项目没有直接复制其完整项目架构，具体实现以本仓库代码为准。

## 开发

安装依赖：

```powershell
yarn install
```

只运行前端：

```powershell
yarn dev
```

运行 Tauri 桌面端：

```powershell
yarn tauri dev
```

构建前端：

```powershell
yarn build
```

构建桌面端：

```powershell
yarn tauri build
```

生成 Tauri 图标：

```powershell
yarn tauri icon CodexSwitch.png
```

## 项目结构

```text
src/                         Vue 前端应用
src/assets/                  前端资源
src/components/              通用组件
src/views/                   页面视图
src/composables/             前端状态和业务组合逻辑
src-tauri/                   Tauri Rust 后端
src-tauri/src/               账号、备份、OAuth、额度、设置等后端模块
src-tauri/icons/             Tauri 应用图标资源
.github/workflows/           GitHub Actions 发布流程
.github/release-notes/       每个版本的更新日志
release/update-policy.json   更新检查策略配置
DESIGN.md                    UI 设计说明
```

## 额度监测说明

额度监测是 best effort 功能，因为 Codex 相关 usage endpoint 不保证是稳定公开 API。

- 额度检查需要用户手动触发。
- 软件不会后台高频轮询额度。
- 如果接口返回 429、401、403 或结构变化，页面只会尽量展示最近错误和恢复时间。

## 发布流程

发布前需要同步版本号：

- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`

为当前版本创建更新日志文件，例如：

```text
.github/release-notes/v0.2.4.md
```

GitHub Actions 会在推送 tag 后读取同名更新日志文件，并将其作为：

- GitHub Release 正文
- Tauri updater 的 `latest.json.notes`

如果找不到同名文件，会回退到：

```text
.github/release-notes/default.md
```

发布示例：

```powershell
git add .
git commit -m "chore: prepare release 0.2.5"
git push origin main

git tag -a v0.2.5 -m "Codex Switch v0.2.5"
git push origin v0.2.5
```

推送 tag 后，GitHub Actions 会创建 Draft Release 并上传 Windows 安装包、Tauri updater artifacts、`latest.json` 和 `update-policy.json`。

确认构建产物无误后，在 GitHub Release 页面发布该 Draft Release。

## 更新策略

启动检查和强制更新策略由 Release 附件中的 `update-policy.json` 控制：

```json
{
  "check_updates_on_startup": true,
  "force_update_on_startup": false,
  "message": "发现新版本时会显示更新内容，你可以选择立即更新或稍后处理。"
}
```

- `check_updates_on_startup`：是否启动时检查更新。
- `force_update_on_startup`：发现新版本后是否要求更新。
- `message`：更新策略说明文案。

默认策略是启动时检查更新，但不强制更新。

## 许可协议

MIT
