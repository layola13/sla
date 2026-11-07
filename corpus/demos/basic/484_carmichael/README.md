# 484 carmichael

Carmichael 函数（`λ(12)=2`；336 的指数版）。

- 对标：336 euler_phi → 素数幂分解取 lcm（`λ(9)=6`）
- 绕行：2 的幂三分段（`1/2/2^(e-2)`）；嵌套 `while` 内层不加 `;`

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/484_carmichael/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/484_carmichael/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/484_carmichael/main.sla -o /tmp/basic_484 && /tmp/basic_484
# 期望输出：2,2,6,6
```
