# 103 Vec Ops

对标 `tsgosa/demos/28_push_unshift`（`2, 3, 18, 7`）。

- 原用例为字符串数组，此处取整数核；`unshift/fill/with` 暂无方法，
  均用显式循环 + `push` + 索引赋值表达（Vec 索引赋值探针已验证双后端）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/103_vec_ops/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/103_vec_ops/main.sla
```
