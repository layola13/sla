# 07 Functions

对标 `tsgosa/demos/07_functions` + `125_fact`。

递归 `fact` + 双参 `add`。与 `tests/test_unit_basic.sla` 的 `factorial` 用例同构，
但这里带 `main + println`，可直接 `build-exe` 跑出 `120 / 42`。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/07_functions/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/07_functions/main.sla
```
