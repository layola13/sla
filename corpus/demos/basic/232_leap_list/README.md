# 232 Leap List

对标 `tsgosa/demos/25_leap` 的表版（2000–2024 闰年数 `7`）。

- 闰年谓词（25 口径）+ 区间计数（61 口径）组合。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/232_leap_list/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/232_leap_list/main.sla
```
