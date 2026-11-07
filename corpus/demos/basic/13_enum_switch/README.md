# 13 Enum Switch

对标 `tsgosa/demos/15_enum_switch`（数字版 `classify`）。

用 `switch c { 0 => {...}, 1 => {...}, default => {...} }` 语句式分支
（见 `tests/test_unit_switch_statement_direct.sla`）。
`enum + switch` 的枚举版见 `demos/rosetta/06_enum_and_match`，此处保持数字版以对齐 tsgosa。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/13_enum_switch/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/13_enum_switch/main.sla
```
