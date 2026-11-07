# 31 Compound

对标 `tsgosa/demos/21_compound`（`x=10` 起步终值 `1`）。

- `+= -= *= /= %= |= &=`：词法原生支持（`src/lexer.zig` 有 `plus_equal/minus_equal/asterisk_equal/slash_equal/percent_equal/pipe_equal/ampersand_equal` token，`src/parser.zig` 脱糖为二元运算+赋值）。
- 本 demo 全部使用原生复合赋值写法，语义与 tsgosa 一致。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/31_compound/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/31_compound/main.sla
```
