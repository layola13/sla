# 33 Switch Return

对标 `tsgosa/demos/111_switch_fn`。

与 `13_enum_switch`（语句赋值式）互补：此处一臂直接 `return`、其余臂赋值
（见 `tests/test_unit_switch_statement_direct.sla` 的混合写法）。
纯全 `return` 臂尾 `switch` 风格现已同样支持（#7 已修复，`check+test` 双绿），
此处保留混合写法与之互补，详见 POTENTIAL_ISSUES #7。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/33_switch_return/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/33_switch_return/main.sla
```
