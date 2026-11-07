# FB-040 函数签名布尔字节宽松读与同记录严格校验并存

- 反馈环：四期 SAB-F 线（Rust 镜像差分）
- 源登记：SAB-D29 解码器镜像时登记的顺带发现（本单为候选，
  未经主会话 minim 确认）
- 分类：(c) 疑似 Zig（sci）缺陷候选——解码面校验不一致
- Rust 侧处置：忠实镜像两处读法（布尔宽松、cap 严格），
  不改判读。

## 机制

sci `src/sab.zig` 的 `readFunctionSigs` / `readTestFunctionSigs`：
签名头部的布尔字段（`is_extern`、`has_ptr_ty`、
`has_return_ty` 等）统一以 `(byte) == 1` 读——任意非 1 字
节静默当 false；而同记录稍后的 `return_cap_present` 却严
格校验 0/1、余值报 `error.InvalidTag`，`return_cap` 走
`readOptionalEnum` 严格枚举校验。畸形流在布尔位写 2 时，
前半段静默吞、后半段报错，失败点与损坏点脱节，不利于
定位。

- 坐标：sci 仓 `src/sab.zig` `readFunctionSigs` /
  `readTestFunctionSigs` 签名头部读法。
- 后端：SAB 读路径。
- 建议形态：布尔字节统一严格化（0/1 两值），或在格式文档
  中明示「布尔位宽松、枚举/cap 位严格」的分工。
