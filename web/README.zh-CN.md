# 本机 Web 宿主

中文 | [English](README.md)

`@wombat/web` 将经过认证的本机 HTTP 请求传递给 `UsageClient`。CLI 提供 Node 客户端，管理宿主的启动和关闭。

## 公开入口

`startWebHost(options)` 返回浏览器地址、origin、异步 `close()` 和受限的 `openView(request)`。

## 限制

宿主只监听回环地址，不支持远程或托管部署。请求必须通过认证，且来源范围必须获得授权。服务重启后，需要使用新的浏览器链接。

## 延伸阅读

- [Web 指南](../docs/guides/web.md)：打开页面与读取失败后的恢复。
- [Web 宿主参考](../docs/reference/web-host.md)：认证、授权、请求限制和取消。
- [架构](../docs/development/architecture.md)：模块边界。
