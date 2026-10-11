# Web 前端

中文 | [English](README.md)

`@wombat/ui` 包含 React、TypeScript 和 Vite 展示代码。`App` 接收注入的 `UsageClient`；浏览器入口连接 HTTP 客户端。

## 职责

UI 展示用量、任务、耗时、配置、检查结果和账户额度。Rust 提供汇总、分组、价格和业务排序。视图保留未知值和覆盖缺口。

## 开发预览

在仓库根目录运行：

```sh
corepack pnpm --filter @wombat/ui preview
```

打开 `http://127.0.0.1:5173/preview.html?page=threads&allTime=1`。预览通过生产组件展示合成数据，不读取来源日志或原生账户。

## 延伸阅读

- [Web 指南](../docs/guides/web.md)：产品启动和恢复。
- [前端开发](../docs/development/frontend.md)：页面、查询生命周期、预览场景和验证限制。
- [产品语言](../docs/i18n/product.md)：语言与文案归属。
