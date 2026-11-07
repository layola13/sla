# 235 Sw Arm

对标 `tsgosa/demos/271_switch_arms`（臂内 `x+10=12` + 暂存分支 `100`）。

- 与 107（臂赋常量）互补，此处臂体用判别式；暂存布尔再分支（209 口径）。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/235_sw_arm/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/235_sw_arm/main.sla
```
