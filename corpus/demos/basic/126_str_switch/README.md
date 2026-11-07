# 126 Str Switch

对标 `tsgosa/demos/498_str_switch`（`1,0`）。

- 字面量 scrutinee + 字面量臂，探针已验证双后端 + exe；
  绑定 scrutinee 的比较语义见缺口 #18，此处只用字面量。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/126_str_switch/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/126_str_switch/main.sla
```
