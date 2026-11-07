# FB-039 readInstructions 的 atomic_value_ty 存在字节宽松读

- 反馈环：四期 SAB-F 线（Rust 镜像差分）
- 源登记：SAB-D29 解码器镜像时登记的顺带发现（本单为候选，
  未经主会话 minim 确认）
- 分类：(c) 疑似 Zig（sci）缺陷候选——解码面校验不一致
- Rust 侧处置：忠实镜像宽松读法（非 1 字节静默当无），
  不改判读。

## 机制

sci `src/sab.zig` 的 `readInstructions`：指令记录的
`atomic_value_ty` 可选字段以一字节存在标记引导，读法为
「读一字节，`== 1` 才继续读枚举值」——任意非 1 字节（2、
0xFF…）被静默当作「无 atomic_value_ty」，解码继续，不报
错。而同一函数的邻近可选字段（`value_ty`、`raw_text` 存在
标记）与 `readOptionalEnum` 均严格校验（非法值
`error.InvalidTag`）。同一记录内两种口径并存，畸形流可
借此把一位移位伪装成合法解码。

- 坐标：sci 仓 `src/sab.zig` `readInstructions`（解码面，
  与 SAB 写路径的 `writeOptionalPrim` 哨兵 0/1 相对）。
- 后端：SAB 读路径（工具面 decodeModule 消费方）。
- 建议形态：存在字节严格化为 0/1 两值，余值 InvalidTag；
  或登记为有意宽松并注释口径。
