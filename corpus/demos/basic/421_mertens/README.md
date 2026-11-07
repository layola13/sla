# 421 mertens

Mertens 和（`M(10)=-1`；387 的累和版）。

- 对标：387 mobius → 逐项累加（`M(5)=-2`）
- 绕行：`0 - 1` 表负数（387 同例）；复用 `mobius`

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/421_mertens/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/421_mertens/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/421_mertens/main.sla -o /tmp/basic_421 && /tmp/basic_421
# 期望输出：-1,-2,1
```
