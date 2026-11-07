# 207 Tern Switch

对标 `tsgosa/demos/942_ternary_switch`（真→`7`，假→`0`）。

- 三元表达式作 `switch` 判别式；与 151（调用结果判别）互补。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/207_tern_switch/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/207_tern_switch/main.sla
```
