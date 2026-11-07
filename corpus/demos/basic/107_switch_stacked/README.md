# 107 Switch Stacked

对标 `tsgosa/demos/282_switch_stacked`（`f(1)+f(2)+f(9)=40`）。

- SLA switch 用 `=>` 胖箭头，不支持 C 式堆叠 `case 1: case 2:`，
  此处以两臂同体表达（与 33/88 口径一致）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/107_switch_stacked/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/107_switch_stacked/main.sla
```
