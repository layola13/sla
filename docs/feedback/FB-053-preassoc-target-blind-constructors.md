# FB-053 — 关联块前构造谓词不查 associated_target：任意 `X::Some/Ok/Err/ptr__null` 都被当构造形接受

- 来源：四期 D-C22 线（sla-plugin-rust checker 镜像），场地现头
  sla `2cc12ea8` 真插件探针实证（docs/probes/dc22/r_some_assoc.sla、
  r_ok_assoc.sla，`sa sla check` rc=0）。
- 分类：Zig 侧缺陷候选 (c)。

## 最小输入

```sla
fn main() -> i32 { let o = foo::Some(42); return 0; }
fn main() -> i32 { let r = foo::Ok(42); return 0; }
```

`foo` 未声明、不存在任何 impl/trait，全文 rc=0：`foo::Some(42)`
被定型为 `Option<i32>`、`foo::Ok(42)` 被定型为
`Result<i32, infer>`。同理 `foo::Err(x)`、`foo::ptr__null<T>()`
（isPtrNullCall 只看 func_name）。

## 机制

`lowering_rules.zig` 的 `isOptionSomeCall` / `isResultOkCall` /
`isResultErrCall` / `isPtrNullCall`（:4252-4285）只比对
`call.func_name`，不查 `call.associated_target`；检查器
`call_expr` 臂（type_checker.zig:8320 起）在关联块**之前**先问
这四个谓词，于是任意带目标的同名调用都被构造臂吞掉，目标名
从不解析（不报 Undefined call、不查目标存在性）。

同段相邻臂口径不一致：`None`（:8323）与 `monitor`（:8328）
都显式要求 `call.associated_target == null`——说明「裸名臂应
限裸调用」是该段自身的既定纪律，构造四谓词漏守。

## 影响面

- 拼写错误的目标（`Optoin::Some(x)` 类）静默通过检查并被赋
  予 Option/Result 类型，错误推迟到 codegen/链接甚至运行期。
- 任何库自定义的 `T::Some` 形关联函数（同名方法）在检查器层
  不可达（被构造臂抢先），且无诊断提示。

## 建议形态

四谓词（或其检查器调用点）补 `associated_target == null`
条件，与 None/monitor 两臂对齐；带目标的同名调用回落到关联
解析级（解析不到即 Undefined call，与 monitor 的
`foo::monitor(h)` rc=1 UndefinedVariable 口径一致）。

## Rust 侧处置

按 no-improvements 纪律照原样镜像（check_preassoc.rs 中
Some/Ok/Err/ptr__null 不查目标），单测
`some_ignores_associated_target` 钉住现行行为；Zig 侧若按
建议修复，镜像随之收紧并改判该测试。
