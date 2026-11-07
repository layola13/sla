# 419 sophie

Sophie Germain 素数（`11 → 23` 是；`<=20 → 4`；366 的倍加版）。

- 对标：366 twin_count → `2p+1` 双素数计数（2/3/5/11）
- 绕行：无（嵌套 `if` 代替 `&&`；复用 `is_prime`）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/419_sophie/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/419_sophie/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/419_sophie/main.sla -o /tmp/basic_419 && /tmp/basic_419
# 期望输出：1,0,4
```
