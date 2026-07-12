# DirDetective 自动更新配置指南

## 密钥管理

### 已生成的密钥

**公钥**（已配置在 `tauri.conf.json`）：
```
dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEM4QTlCNEJBOTdCMEZGQUQKUldTdC83Q1h1clNweUJFMG1xTWZIYW1iMFBEZzZwdUliNTFJY2xuY1RMVm5uV2VJUERTaFEvQjAK
```

**私钥**（存储在 `gui/.env.secret`，不要提交到 Git）：
```
TAURI_SIGNING_PRIVATE_KEY=dW50cnVzdGVkIGNvbW1lbnQ6IHJzaWduIGVuY3J5cHRlZCBzZWNyZXQga2V5...
TAURI_SIGNING_PRIVATE_KEY_PASSWORD=dirdetective
```

## 发布更新

### 1. 准备发布

确保已设置环境变量：
```bash
cd gui
source .env.secret  # 或者手动导出环境变量
```

### 2. 更新版本号

在以下文件中更新版本号：
- `gui/src-tauri/tauri.conf.json` 中的 `version`
- `gui/src-tauri/Cargo.toml` 中的 `version`

### 3. 构建并签名

```bash
cd gui
pnpm tauri build
```

这将生成：
- **安装包**：`.dmg` (macOS), `.exe` (Windows), `.AppImage` (Linux)
- **签名文件**：`.sig` 文件
- **更新清单**：`latest.json`

### 4. 发布到 GitHub

```bash
# 创建 Git 标签
git tag v0.1.0
git push origin v0.1.0

# 或者使用 GitHub Releases 网页上传文件
```

### 5. 上传更新文件

在 GitHub Releases 页面上传以下文件：
- `latest.json` - 更新清单
- `DirDetective_<version>_x64.dmg` - macOS 安装包
- `DirDetective_<version>_x64.dmg.sig` - macOS 签名
- `DirDetective_<version>_x64.exe` - Windows 安装包
- `DirDetective_<version>_x64.exe.sig` - Windows 签名
- `DirDetective_<version>_amd64.AppImage` - Linux 安装包
- `DirDetective_<version>_amd64.AppImage.sig` - Linux 签名

## 用户更新流程

1. 应用启动时自动检查更新（或在设置中手动检查）
2. 发现新版本时显示下载按钮
3. 点击下载后自动下载更新包（显示进度）
4. 下载完成后自动安装
5. 重启应用完成更新

## 安全注意事项

⚠️ **重要安全提示：**

1. **私钥保护**：私钥文件 `.env.secret` 绝对不能提交到 Git
2. **密钥备份**：将私钥备份到安全的地方（如密码管理器）
3. **密码保护**：私钥使用密码 `dirdetective` 加密
4. **密钥轮换**：如果私钥泄露，需要立即重新生成并更新公钥

## 重新生成密钥

如果需要重新生成密钥：

```bash
cd gui
pnpm tauri signer generate --ci -p "your-new-password"
```

然后更新：
1. `tauri.conf.json` 中的 `pubkey`
2. `.env.secret` 中的私钥
3. 重新构建所有版本

## 验证更新

测试更新功能：

1. 构建旧版本（如 v0.1.0）并安装
2. 构建新版本（如 v0.2.0）并发布
3. 在旧版本中点击"检查更新"
4. 验证下载、安装和重启流程

## 故障排除

### 更新检查失败
- 检查网络连接
- 验证 GitHub Releases URL 正确
- 查看浏览器控制台错误日志

### 签名验证失败
- 确认公钥配置正确
- 检查签名文件是否存在
- 验证私钥密码正确

### 下载失败
- 确认 GitHub Releases 上传了所有文件
- 检查文件大小是否正常
- 验证 `latest.json` 格式正确
