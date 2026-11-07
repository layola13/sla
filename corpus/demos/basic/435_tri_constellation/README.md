# 435 tri constellation

素数星座三向计数（`<=50 → 6,7,2`；366/430/431 合取）。

- 对标：431 prime_triplet → 孪生/三元/四元同界并列（`6,7,2`）
- 绕行：深嵌套 `if` 逐层收窄（代替四元 `&&`）；三计数器独立

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/435_tri_constellation/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/435_tri_constellation/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/435_tri_constellation/main.sla -o /tmp/basic_435 && /tmp/basic_435
# 期望输出：6,7,2
```
