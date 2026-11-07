# 789 duel_schro9

大 Schröder 续高位对峙：迭代 vs 商式（`n=8：41586 对 41586`；`n=9：206098 对 206098`）。
商式 `S(n)=sum C(n+k,k)·C(n,k)/(k+1)`（`n=9` 十项和 `206098`）。

- 对标：786_schro9 → 迭代源（商式版互补；286 系对峙体例，双边同值锁定）
- 绕行：商式逐项精确整除（596 口径；探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/789_duel_schro9/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/789_duel_schro9/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/789_duel_schro9/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/789_duel_schro9/main.sla -o /tmp/basic_789 && /tmp/basic_789
# 期望输出：41586,41586,206098,206098
```
