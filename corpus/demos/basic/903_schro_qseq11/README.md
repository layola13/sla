# 903 schro_qseq11

大 Schröder 商式纵深 `qsch(10)=1037718`，`qsch(11)=5293446`（商式上移一位）。

- 对标：793_schro_qseq → `n=7/8/9` 版互补（商式纵深四值锁定；905 做三向收束）
- 绕行：商式逐项精确整除（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/903_schro_qseq11/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/903_schro_qseq11/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/903_schro_qseq11/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/903_schro_qseq11/main.sla -o /tmp/basic_903 && /tmp/basic_903
# 期望输出：1037718,5293446,206098
```
