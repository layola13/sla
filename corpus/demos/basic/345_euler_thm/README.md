# 345 Euler Thm

欧拉定理验证（`2^φ(5) mod 5=1`，另断言 `3^φ(7)`）。

- 模幂（328 口径）×欧拉函数（336 口径）合取。

```bash
SA_PLUGIN_DEV=1 sa sla check demos/basic/345_euler_thm/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/345_euler_thm/main.sla
```
