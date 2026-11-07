# 589 lah_rec

Lah 递推版 `L(n,k)=L(n-1,k-1)+(n+k-1)·L(n-1,k)`（拍扁表）。
`L(4,2)=36`，`L(5,2)=240`，`L(5,3)=120`。

- 对标：586_lah → 商式版互补（此处为加权递推，531 范式；590 做对峙）
- 绕行：权 `(nn+kk-1)` 双下标和式（531 单权版扩展，一次绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/589_lah_rec/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/589_lah_rec/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/589_lah_rec/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/589_lah_rec/main.sla -o /tmp/basic_589 && /tmp/basic_589
# 期望输出：36,240,120
```
