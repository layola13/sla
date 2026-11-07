# 816 stir19

Stirling 一类第 9 行左部 `c(9,2)=109584`，`c(9,3)=118124`，`c(9,4)=67284`（同式再上三行）。

- 对标：686_stir17 → `n=7` 版互补（同式再上三行；819 做建表/递归对峙）
- 绕行：`13068+8·13132=118124` 双项手算复核（探针三线绿）

```bash
export PATH="/content/sa_all/sci/zig-out/bin:/tmp/gotools/go/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/816_stir19/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/816_stir19/main.sla --test-backend sab
SA_PLUGIN_DEV=1 sa sla test demos/basic/816_stir19/main.sla --test-backend sa
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/816_stir19/main.sla -o /tmp/basic_816 && /tmp/basic_816
# 期望输出：109584,118124,67284
```
