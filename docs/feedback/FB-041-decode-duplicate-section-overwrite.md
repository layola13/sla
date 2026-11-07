# FB-041 decodeModule 重复段静默覆盖

- 反馈环：四期 SAB-F 线（Rust 镜像差分）
- 源登记：SAB-D29 解码器镜像时登记的顺带发现（本单为候选，
  未经主会话 minim 确认）
- 分类：(c) 疑似 Zig（sci）缺陷候选——解码面无诊断
- Rust 侧处置：忠实镜像「后者覆盖」语义，不加诊断。

## 机制

sci `src/sab.zig` 的 `decodeModule` 按 (section id, len)
顺序遍历段体，逐段填充 `Module` 字段；同一 section id 出
现两次时，第二次解码结果直接覆盖第一次，无任何诊断（写
路径从不产生重复段，重复段必为手工构造/损坏产物）。与
之相对，未知 section id 同样静默跳过——后者可辩护为前向
兼容，前者覆盖的却是格式内已定义语义段，两次解码之间
先解码的签名/常量被整体丢弃，消费方无从察觉。

- 坐标：sci 仓 `src/sab.zig` `decodeModule` 段循环。
- 后端：SAB 读路径。
- 建议形态：已定义 section id 重复出现时报
  `error.DuplicateSection`（或至少留诊断标记）；未知 id
  的跳过保持。
