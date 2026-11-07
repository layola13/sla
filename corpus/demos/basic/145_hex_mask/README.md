# 145 Hex Mask

对标 `tsgosa/demos/1250_mask_idioms`（`255,511,17`）。

- SLA 支持 `0x` 前缀（`0o`/`0b` 不可解析，InvalidCharacter）；
  注意优先级：`&`/`|` 低于 `!=`，条件中须写 `(x & 0xFF) != 255`。
  与 76 的位运算口径一致，此处主覆盖字面量进制。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/145_hex_mask/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/145_hex_mask/main.sla
```
