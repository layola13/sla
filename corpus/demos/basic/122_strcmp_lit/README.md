# 122 Strcmp Lit

对标 `tsgosa/demos/384_strcmp`（`-1,1,0,-1,1`）。

- 缺口 #18：字符串关系比较的临时量在 `@test` 体内直接求值会泄漏
  （SAB 报 MemoryLeak trap；绑定左值在 exe 中亦失真），此处将每次比较
  包进无参 helper fn（字面量比较 + `str_eq`，探针已验证双后端 + exe）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/122_strcmp_lit/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/122_strcmp_lit/main.sla
```
