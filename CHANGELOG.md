# 更新日志

本文件的格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [语义化版本 2.0.0](https://semver.org/lang/zh-CN/)。

## [未发布]

### 新增
- 下载页搜索结果点击进入模组详情页，可选择具体模组版本后下载到实例
- `add` 子命令新增 `--version` 参数，可指定 Modrinth 项目版本（按版本 ID 或版本号）
- 搜索结果列表显示资源图标，加载失败或缺失时显示首字符占位
- 下载页添加内容时实例名称支持点选已有实例，无需手动输入
- 下载页搜索结果直接展示在界面中，点选即可选中项目并添加到实例下载
- 安装页支持搜索选择游戏版本与加载器版本，列表来自 Mojang 版本清单与各加载器官方源
- 微软登录自动复制验证码并打开浏览器，粘贴后即可完成授权

### 变更
- 下载页整合包标签页改为"搜索 → 整合包详情 → 选版本 → 导入"流程：搜索结果点击进入
  整合包详情页，可选择具体整合包版本并填写新实例名称后导入，移除手填"项目 slug"面板
- 下载页模组、资源包、光影包、数据包标签页改为"搜索 → 模组详情 → 下载"一体化流程，
  移除独立的"添加"面板，实例名称选择移入搜索面板
- 下载页搜索的实例版本改为自动取自所选实例：模组、资源包、光影包、数据包
  标签页移除手动"实例版本"输入，界面只读展示检测到的版本
- 下载页"光影"标签更名为"光影包"，三种语言同步更新

### 修复
- 下载页"游戏版本"与安装页"实例版本"叫法并存的问题：统一更名为"实例版本"
- 下载页整合包标签页的实例版本输入缺少搜索列表：与安装页一致，支持展开列表搜索选择版本
- Windows 下双击启动图形界面时不再残留控制台窗口
- 模组实例添加光影包、资源包或数据包时提示 `no matching version` 导致下载失败：
  安装此类内容不再套用实例的模组加载器过滤

## [v0.0.4-alpha] - 2026-11-15

### 移除
- 模组批量更新：移除 `mods update` 子命令及 Modrinth `/version_files/update` 接口

### 修复
- Linux 下图形界面无法显示：启用 `gpui-fast-platform` 的 wayland 与 x11 显示后端

## [v0.0.3-alpha] - 2026-11-14

### 新增
- 正版登录：微软设备码流程（XBL → XSTS → Minecraft），新增 `login` / `logout` 子命令，
  令牌自动续期；启动时已登录则自动使用正版身份，`--offline` / `--username` 可强制离线
- 图形界面设置页新增账号区块：显示当前账号，支持发起登录与退出登录任务
- 模组管理：新增 `mods list|remove|toggle|update` 子命令与实例设置页模组面板，
  支持列出（jar 元数据解析 + Modrinth 来源识别）、启停（`.jar.disabled` 后缀）、
  按名删除与 Modrinth 来源模组的一键批量更新

### 变更
- 设置页与下载页的路径输入支持通过文件对话框选择

### 修复
- 命令行未知参数不再被静默忽略，改为报错并退出
- Java 路径填写 JDK 目录或空值时无法定位 java 可执行文件
- 离开实例设置页后仍保留上次实例名，任务完成时会刷新旧实例的模组列表
- 实例设置保存按钮改为固定在页面底部，不再随内容滚动移出视野

## [v0.0.2-alpha] - 2026-11-14

### 新增
- 多语言（i18n）支持：新增简体中文 / English / 日本語三套界面文案，缺失键依次回退
  en-US 与键名本身，界面语言可在全局配置中切换
- 图形界面多任务模型：重构任务体系并新增任务页面，支持多任务并行执行、
  逐行实时日志与任务取消
- 下载页重组：支持搜索 Modrinth 整合包并直接导入

### 变更
- 界面重构为暗色极简双栏布局
  （[#1](https://github.com/Zecheng-6114/lyys_mc_launcher/pull/1)）
- 下载页加载器由文本输入改为选择器，并精简表单文案
- 新增英文与日文 README，三种语言页面互相跳转
- 项目以 GPL-3.0 许可证发布，并清理源码注释
- 升级核心依赖

## [v0.0.1-alpha] - 2026-11-11

首个发布版本。

### 新增
- 双模式入口：无参数启动 GPUI 图形界面，带参数进入命令行模式
  （launch / download / search / add / import 五个子命令）
- 版本安装：一键安装原版，或叠加 Fabric / Forge / NeoForge 加载器
- 启动流程：解析 inheritsFrom 继承链并合并库与启动参数，补全版本文件与资源，
  准备 natives 后拉起游戏
- 整合包导入：支持 Modrinth（.mrpack）、CurseForge、MCBBS 与服务端整合包，
  支持 --dry-run 预览
- 模组管理：通过 Modrinth API 搜索并安装 mod，自动处理依赖（可选依赖可控）
- 实例隔离：每个实例可独立使用自己的 versions/<实例> 目录作为游戏目录
- 下载完整性：SHA1 / SHA512 校验与失败重试，支持 --mirror / --maven 镜像加速
- 离线启动：由用户名派生离线 UUID
- 构建发布工作流：推送 v* 标签时在 Windows / Linux 构建并发布

### 修复
- 适配 zip 9 API 并升级依赖

[未发布]: https://github.com/Zecheng-6114/lyys_mc_launcher/compare/v0.0.4-alpha...HEAD
[v0.0.4-alpha]: https://github.com/Zecheng-6114/lyys_mc_launcher/compare/v0.0.3-alpha...v0.0.4-alpha
[v0.0.3-alpha]: https://github.com/Zecheng-6114/lyys_mc_launcher/compare/v0.0.2-alpha...v0.0.3-alpha
[v0.0.2-alpha]: https://github.com/Zecheng-6114/lyys_mc_launcher/compare/v0.0.1-alpha...v0.0.2-alpha
[v0.0.1-alpha]: https://github.com/Zecheng-6114/lyys_mc_launcher/releases/tag/v0.0.1-alpha
