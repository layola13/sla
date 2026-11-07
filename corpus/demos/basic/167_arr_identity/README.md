# 167 Arr Identity

对标 `tsgosa/demos/1202_arr_identity` + `1203_self_identity`（`0,1`）。

- 数组 `==` 为引用恒等（同绑定才真），内容相同但绑定不同则假
  （与 TS 语义一致，双后端 + exe 核对）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/167_arr_identity/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/167_arr_identity/main.sla
```
