# 415 duel harshad

Harshad 双实现对峙（循环版 vs 递归版；411 的整除版）。

- 对标：411 duel_dsum → 整除判定双边锁定（`18 → 1=1`）
- 绕行：`n < 1` 双边守卫（`dsum(0)=0` 除零坑，410 同例）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/415_duel_harshad/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/415_duel_harshad/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/415_duel_harshad/main.sla -o /tmp/basic_415 && /tmp/basic_415
# 期望输出：1,1,0
```
