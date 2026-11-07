# 15 Option

对标 `tsgosa/demos/18_optional`（`?. ??`）。

SLA 用 `Option::Some/None + match` 表达空值语义。注意收敛值臂若移动绑定
（两臂都返回绑定的值），SA-text 路径可能报 `PhiStateConflict`
（见 `tests/test_unit_option_match.sla` 头注，属上游 SA-text 已知限制），
此处一臂为 `v + 1`（表达式值）以保持双路径可验证。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/15_option/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/15_option/main.sla
```
