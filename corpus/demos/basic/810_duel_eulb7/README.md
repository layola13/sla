# 810 duel_eulb7

Eulerian-B 续高位二对峙：建表 vs 交错和（`(7,2)：60657 对 60657`；`(7,3)：259723 对 259723`）。

- 对标：808_eulb7 → 高位源互补（809 对峙体例上移；双边同值锁定）
- 绕行：`(7,3)` 交错四项 `823543-625000+61236-56=259723` 手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/810_duel_eulb7/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/810_duel_eulb7/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/810_duel_eulb7/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/810_duel_eulb7/main.sla -o /tmp/basic_810 && /tmp/basic_810
# 期望输出：60657,60657,259723,259723
```
