# Codex Switch 更新发布流程

本文档记录 Codex Switch 的标准更新发布流程。目标是让每次发布都能同时更新：

- 应用内部版本号
- GitHub Release 正文
- Tauri updater 的 `latest.json.notes`
- Windows 安装包
- `update-policy.json` 更新策略文件

## 发布机制概览

Codex Switch 使用 GitHub Actions + Tauri updater 发布 Windows 安装包。

流程核心是：

1. 本地准备版本号和更新日志。
2. 推送 `main`。
3. 创建并推送 `vX.Y.Z` tag。
4. GitHub Actions 构建 Windows 安装包。
5. Actions 创建 Draft Release。
6. 审核 Draft Release 产物。
7. 手动 Publish Release。
8. 已安装用户启动软件后读取 `latest.json` 并看到更新弹窗。

## 版本号文件

每次发布前必须同步以下文件中的版本号：

```text
package.json
src-tauri/tauri.conf.json
src-tauri/Cargo.toml
src-tauri/Cargo.lock
```

版本号格式使用语义化版本：

```text
0.2.4
```

Git tag 使用 `v` 前缀：

```text
v0.2.4
```

## 更新日志文件

每个版本都应该在发布前创建同名更新日志：

```text
.github/release-notes/vX.Y.Z.md
```

例如：

```text
.github/release-notes/v0.2.4.md
```

这个文件会被 GitHub Actions 读取，并作为：

- GitHub Release 正文
- Tauri updater 生成的 `latest.json.notes`

如果当前 tag 没有对应的更新日志文件，workflow 会回退到：

```text
.github/release-notes/default.md
```

为了避免发布说明过于笼统，正式版本不要依赖 `default.md`。

## 更新日志建议格式

推荐结构：

```md
## Codex Switch v0.2.5

这是一个简短版本概述。

### 新增

- 新增功能。

### 改进

- 改进点。

### 修复

- 修复的问题。

### 验证

- Rust 后端测试通过：`15 passed`
- 前端生产构建通过：`yarn build`
- Tauri release 构建通过：`yarn tauri build --no-bundle`
```

只保留实际发生的栏目，不需要为了格式强行写空章节。

## 更新策略文件

更新策略由以下文件控制：

```text
release/update-policy.json
```

示例：

```json
{
  "check_updates_on_startup": true,
  "force_update_on_startup": false,
  "message": "发现新版本时会显示更新内容，你可以选择立即更新或稍后处理。"
}
```

字段说明：

- `check_updates_on_startup`：是否启动软件时检查更新。
- `force_update_on_startup`：发现新版本后是否要求更新。
- `message`：更新策略说明文案。

默认建议：

```json
{
  "check_updates_on_startup": true,
  "force_update_on_startup": false
}
```

只有在存在必须升级的严重问题时，才考虑设置：

```json
"force_update_on_startup": true
```

## 本地发布前检查

发布前建议先确认工作树状态：

```powershell
git status --short
```

如果只有明确要发布的文件变化，继续执行验证。

标准验证命令：

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
yarn build
yarn tauri build --no-bundle
```

验证目标：

- Rust 测试全部通过。
- Vue/TypeScript 构建通过。
- Tauri release 可编译。
- 构建输出中的 crate 版本和本次版本一致。

示例输出中应看到：

```text
Compiling codex-account-switcher v0.2.5
```

## 提交 main

确认版本号、更新日志和代码改动无误后提交：

```powershell
git add package.json src-tauri\tauri.conf.json src-tauri\Cargo.toml src-tauri\Cargo.lock .github\release-notes\v0.2.5.md
git commit -m "chore: prepare release 0.2.5"
git push origin main
```

如果本次发布包含代码修复或资源修改，也要把对应文件加入提交。

不要提交本地临时文件，例如：

```text
logo.psd
qa-exe-icon.png
```

## 创建并推送 tag

使用 annotated tag：

```powershell
git tag -a v0.2.5 -m "Codex Switch v0.2.5"
git push origin v0.2.5
```

推送 tag 后，GitHub Actions 会自动开始构建。

## GitHub Actions 构建流程

workflow 文件：

```text
.github/workflows/release.yml
```

关键步骤：

1. Checkout 代码。
2. 安装 Node/Yarn。
3. 启用 Corepack。
4. 安装 Rust。
5. 安装前端依赖。
6. 读取 `.github/release-notes/<tag>.md`。
7. 运行 `tauri-apps/tauri-action` 构建 Windows 安装包。
8. 创建 Draft Release。
9. 上传 `release/update-policy.json`。

workflow 已设置：

```yaml
FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true
```

用于提前适配 GitHub Actions 的 Node.js 24 运行环境。

## Draft Release 审核

Actions 完成后，打开 GitHub Releases 页面：

```text
https://github.com/9ycrooked/CodexSwitch/releases
```

找到新版本 Draft Release，检查：

- Release 标题是否正确，例如 `Codex Switch v0.2.5`。
- Release 正文是否来自 `.github/release-notes/v0.2.5.md`。
- Windows 安装包是否上传成功。
- `latest.json` 是否存在。
- `update-policy.json` 是否存在。
- updater signature 是否存在。

常见附件包括：

```text
Codex.Switch_0.2.5_x64_zh-CN.msi
Codex.Switch_0.2.5_x64-setup.exe
latest.json
update-policy.json
```

确认无误后，点击 Publish Release。

## latest.json 检查

`latest.json` 是 Tauri updater 读取的机器配置文件。

关键字段：

```json
{
  "version": "0.2.5",
  "notes": "更新日志内容",
  "pub_date": "发布时间",
  "platforms": {
    "windows-x86_64": {
      "signature": "安装包签名",
      "url": "安装包下载地址"
    }
  }
}
```

检查点：

- `version` 必须等于当前发布版本。
- `notes` 应该等于当前 release notes 文件内容。
- `url` 应该指向当前 tag 的安装包。
- `signature` 不能手动乱改。

不要手动编辑 `signature` 字段。

## 发布后验证

发布后建议做一次安装和更新验证：

1. 安装旧版本。
2. 启动旧版本。
3. 确认能检查到新版本。
4. 查看更新弹窗内容是否正确。
5. 点击更新并安装。
6. 重启后确认版本号和功能正常。

如果更新弹窗内容不对，优先检查：

- `.github/release-notes/vX.Y.Z.md`
- Draft Release 正文
- `latest.json.notes`

如果安装失败，优先检查：

- 安装包是否存在。
- `latest.json.platforms.*.url` 是否指向当前版本。
- `signature` 是否和安装包匹配。
- 应用是否正在运行导致安装包无法替换文件。

## 重新发布同一个 tag

通常不建议重复移动已发布 tag。

如果必须修正同一个 tag：

```powershell
git tag -d v0.2.5
git push origin :refs/tags/v0.2.5

git tag -a v0.2.5 -m "Codex Switch v0.2.5"
git push origin v0.2.5
```

注意：

- 这会重新触发 GitHub Actions。
- 已经下载过旧 `latest.json` 的用户可能遇到缓存或版本混乱。
- 更推荐发布新的补丁版本，例如 `v0.2.6`。

## 图标更新流程

如果需要替换应用图标：

1. 更新根目录源图：

```text
CodexSwitch.png
```

2. 生成 Tauri 图标资源：

```powershell
yarn tauri icon CodexSwitch.png
```

3. 如果自定义标题栏也要使用该图标，需要同步前端资源：

```text
src/assets/CodexSwitch.png
src/components/AppTitlebar.vue
src/styles/layout.css
```

4. 运行发布前验证。

Windows 任务栏或快捷方式图标可能存在缓存。如果构建产物中的 exe 图标正确，但系统仍显示旧图标，优先尝试：

- 取消任务栏固定并重新固定。
- 删除旧桌面快捷方式。
- 卸载旧版本后重新安装。
- 重启 Windows 资源管理器。

## 常见问题

### GitHub Actions 提示 Node.js 20 deprecated

workflow 已设置：

```yaml
FORCE_JAVASCRIPT_ACTIONS_TO_NODE24: true
```

如果未来 action 官方升级到新版，可以再考虑把：

```yaml
actions/checkout@v4
actions/setup-node@v4
```

升级到更新主版本。

### 更新安装失败：Cannot read private member from an object whose class did not declare it

这是 Vue 响应式代理 Tauri updater 对象导致的问题。

修复方式是使用：

```ts
shallowRef
markRaw
```

确保 `Update` 对象不被 Vue Proxy 包装。

### 软件内更新文案和 GitHub Release 正文不一致

检查 workflow 是否读取了正确文件：

```text
.github/release-notes/<tag>.md
```

再检查 Draft Release 附件中的：

```text
latest.json
```

其中 `notes` 应该和 release notes 文件一致。

## 发布检查清单

发布前：

- [ ] 版本号已同步到 4 个版本文件。
- [ ] 已创建 `.github/release-notes/vX.Y.Z.md`。
- [ ] `release/update-policy.json` 已按需要调整。
- [ ] `cargo test --manifest-path src-tauri\Cargo.toml` 通过。
- [ ] `yarn build` 通过。
- [ ] `yarn tauri build --no-bundle` 通过。
- [ ] `git status --short` 没有意外文件。

发布中：

- [ ] 已推送 `main`。
- [ ] 已创建 annotated tag。
- [ ] 已推送 tag。
- [ ] GitHub Actions 构建成功。
- [ ] Draft Release 附件齐全。
- [ ] Release 正文正确。
- [ ] `latest.json` 内容正确。

发布后：

- [ ] 已 Publish Release。
- [ ] 旧版本能检测到新版本。
- [ ] 软件内更新弹窗显示正确。
- [ ] 更新安装能完成。
- [ ] 更新后应用能正常启动。
