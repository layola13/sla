# 119 Shift

对标 `tsgosa/demos/372_ushr` + `412_shift_compound`（`4,-4,32,32`）。

- SLA 有 `>>`/`<<`（算术右移），无 `>>>`（parser 拒绝第三个 `>`），
  无 `<<=`（无词法 token，见 #6 同类），用 `a = a << 2` 表达。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/119_shift/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/119_shift/main.sla
```
