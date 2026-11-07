# 83 Slice Concat

对标 `tsgosa/demos/27_slice_concat`（切片求值 `9` + 拼接求值 `17`）。

- SLA 暂无 `slice/concat` 方法，均用 `Vec::push` 手工拷贝表达（写法见 `39/53`）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/83_slice_concat/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/83_slice_concat/main.sla
```
