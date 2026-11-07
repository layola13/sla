# 784 bell_parity

Bell 奇偶性 Touchard 同余 `B(n) mod 2` 周期 3（`B(8)=0`，`B(9)=1`，`B(10)=1`）。

- 对标：776_bell9 → 单值版互补（数论性质单锁；`%2` 判奇偶 515 口径）
- 绕行：`877%2=1` 小值守卫；周期 `1,1,0` 三值连验（差值输出避布尔直打 493 口径）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/784_bell_parity/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/784_bell_parity/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/784_bell_parity/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/784_bell_parity/main.sla -o /tmp/basic_784 && /tmp/basic_784
# 期望输出：0,1,1
```
