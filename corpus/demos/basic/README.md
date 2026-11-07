# SLA Basic Demos（对标 tsgosa）

本目录把 `tsgosa/demos` 中“小而渐进”的基础用例移植为 SLA 写法，
与 `demos/rosetta`（Rust 语义对照，重型）形成互补：
`basic` 聚焦语言基本功 + 常用算法，每一个 demo 都可独立 `check` / `test`。

- 参考源：`/content/sa_all/tsgosa/demos/01_hello` … `200_finale`（TS 版 `main.ts` + `expected.stdout`）
- SLA 写法：`fn main() -> i32`（`println` 演示）+ `@test`（断言语义，等价于 `expected.stdout`）
- Rust 对照仍在 `demos/rosetta`，这里不再重复 Rust，README 会标注对应的 tsgosa 编号。

## Commands

```bash
export PATH="/tmp/gotools/zig-x86_64-linux-0.14.1:/content/sa_all/sci/zig-out/bin:$PATH"
SA_PLUGIN_DEV=1 sa sla check demos/basic/01_hello/main.sla
SA_PLUGIN_DEV=1 sa sla test demos/basic/01_hello/main.sla
SA_PLUGIN_DEV=1 sa sla build-exe demos/basic/01_hello/main.sla -o /tmp/basic_01_hello && /tmp/basic_01_hello
```

批量验证（bash）：

```bash
for d in demos/basic/*/; do echo "== $d"; SA_PLUGIN_DEV=1 sa sla check "$d/main.sla" || break; done
for d in demos/basic/*/; do echo "== $d"; SA_PLUGIN_DEV=1 sa sla test "$d/main.sla" || break; done
```

## Index

| SLA demo | 对标 tsgosa | 覆盖点 |
|---|---|---|
| 01_hello | 01_hello / 20_console | `println`，`main -> i32` |
| 02_arith | 02_arith / 21_compound | `+ - * / %`，负数，优先级 |
| 03_if_else | 03_if_else | `if/else`，`max` 函数 |
| 04_while_sum | 04_while_sum / 110_while_break | `while`，累加 |
| 05_for_sum | 05_for_sum | `for in 0..n`，`%` 过滤 |
| 06_ternary | 06_ternary / 153_ternary_chain | `if` 表达式（三元） |
| 07_functions | 07_functions / 125_fact | 递归 `fact`，多函数 |
| 08_arrays | 08_arrays | 定长数组 `[int; N]` 读写 |
| 09_vec_methods | 09_array_methods | `Vec::push/len/remove`，循环求和 |
| 10_strings | 10_strings / 16_template | `println` 格式化，`str_eq` |
| 11_struct | 13_struct / 19_destructure | `struct` 构造，字段读写 |
| 12_impl_counter | 14_class | `struct + impl`（`Counter::inc`） |
| 13_enum_switch | 15_enum_switch | `enum + switch`，`classify` |
| 14_bool_logic | 17_bool_logic | `&& \|\| !` |
| 15_option | 18_optional | `Option::Some/None`，`match` |
| 16_insert_sort | 102_insert_sort | 插入排序（`while` 内移） |
| 17_bsearch_gcd | 101_bsearch / 119_gcd3 | 二分查找 + 辗转相除 |
| 18_fib_sieve | 173_fib_loop / 128_sieve | fib 循环，筛法计数 |
| 19_matrix | 143_mat_add / 144_transpose | 2x2 矩阵加法 + 转置求和 |
| 20_closures | 109_closure / 24_find_some_every | 闭包捕获，`find/some` 手写循环版 |
| 21_gcd_lcm | 120_lcm | 递归 `gcd` + `lcm` |
| 22_second_max | 121_second_max | 一遍扫描最大/次大，`else if` 链 |
| 23_rotate | 122_rotate | 左旋（取模索引，无 `slice/concat`） |
| 24_fizzbuzz | 191_fizz20 | `for 1..(n+1)` + `||` 整除计数 |
| 25_leap | 180_leap | 多 `if` 早返回闰年判断 |
| 26_clamp_max3 | 126_clamp | `if` 表达式链代替 `Math.min/max` |
| 27_sum_sq_dot | 141_sum_sq / 142_dot | 平方和 + 点积 |
| 28_mat_mul | 184_matrix_mul | 2x2 乘法对角元 |
| 29_sum_even_swap | 192_sum_even / 195_swap | 偶数求和 + 三变量交换 |
| 30_palindrome | 103_palindrome | int 数组回文（字符串索引有坑，见 POTENTIAL_ISSUES #3/#5） |
| 31_compound | 21_compound | `+= -= *= /= %= \|= &=` 全原生（#6 已修复） |
| 32_while_break | 110_while_break | `while true + break/continue` |
| 33_switch_return | 111_switch_fn | 混合写法保留，纯全 `return` 臂亦支持（#7 已修复） |
| 34_pick_max | 112_pick_max | `if + 早返回`（与 03 表达式式互补） |
| 35_min_range | 116_minloop / 129_range | 最小值 + 区间求和 |
| 36_prefix | 118_prefix | `^borrow` 原地前缀和 |
| 37_mode | 146_mode | 嵌套循环众数 |
| 38_median | 147_median | 插入排序后取中位数（无 `toSorted`） |
| 39_vec_concat_dedup | 136_concat_all / 117_dedup | `Vec` 拼接 + 手写 `contains` 去重 |
| 40_do_sum | 158_do_sum | `while true + break` 表达 `do-while` 语义 |
| 41_merge_sorted | 123_merge | 双指针归并 + `&&` 收尾 |
| 42_every_positive | 127_sorted | 定长数组手写 `every` |
| 43_tally_even | 140_tally | 偶数计数 + 求和 |
| 44_bit_count | 148_bit_count | `&` + `>>` 置位计数 |
| 45_divmod_maxmin | 179_div_mod / 186_max3 | 整除取余 + 手写 max/min |
| 46_prime_upto | 183_prime_upto | `for + break` 剪枝，30 以内 10 个 |
| 47_default_args | 150_multi_default | 默认参数用双函数表达（见 POTENTIAL_ISSUES #8） |
| 48_nested_break | 157_label_nested | 内层 `break` 代替 label（见 POTENTIAL_ISSUES #9） |
| 49_collatz_pow | 155_collatz / 135_pow_loop | collatz(27)=111（计数口径已核对）+ 2^10 |
| 50_nested_sign_min_step | 189_nest_if3 / 175_min3 / 199_for_step2 | 嵌套 `if` 符号函数 + min3 + `while` 步长求和（`for` 无步长） |
| 51_gcd_sum | 159_gcd_sum | `while` 版 `gcd` + `gcd(i,12)` 累加（=27） |
| 52_pow2_series | 160_pow2 | 翻倍循环 2 的幂 `Vec`（无 `**`，8/1/128） |
| 53_concat3 | 161_concat3 | `Vec::push` 三段拼接（无 `concat`，4/1/4） |
| 54_neg_index | 162_at_neg / 169_neg_idx | `a[len-1]` 表达 `at(-1)`（无负索引） |
| 55_num_sep | 164_num_sep | 普通字面量（数字分隔符见缺口 #11） |
| 56_chain_ops | 165_chain | 顺序赋值表达链式赋值（缺口 #12） |
| 57_iife | 170_iife | 先绑定再调用（直接调用见缺口 #13） |
| 58_sum_2d | 172_sum_2d | 扁平 `[int; 6]` 双层循环（无嵌套字面量） |
| 59_count_even | 174_count_even | `Vec` 版偶数计数（与 29/43 互补） |
| 60_sort_desc | 190_sort_desc | 取反比较子降序插入（与 16/38 互补） |
| 61_while_sum2 | 198_while_sum2 | `while` 倒数累加（与 04/05 互补） |
| 62_pow_sum | 176_pow_sum | 翻倍累乘 2 的幂求和（无 `**`，15） |
| 63_binary | 187_binary | 循环组装二进制 `01010101b=85`（与 44 互补） |
| 64_select | 188_select | 变量索引取值（`a[1]=20`，`a[2]=30`） |
| 65_sort_slice | 178_obj_sum | 插入排序 + 手工拷贝切片（无 `slice`） |
| 66_pair_sum | 194_class_pair | 双字段 `struct+impl` + 求和方法 |
| 67_map_dbl | 193_map_dbl | 显式循环 `push` 表达 `map` 翻倍 |
| 68_str_count | 185_str_join2 | `for-of` 改直列 `str_eq` 计数（无迭代器协议） |
| 69_len_sum | 196_str_len_sum | 字面量 + 变量 `len` 求和（=10，缺口 #10 已修复） |
| 70_finale_mini | 200_finale | 递归 `fib(10)` + 排序首尾和收官综合 |
| 71_tri | 137_tri | `for` 正序三角数累加（=55，与 04/05/61 互补） |
| 72_diag | 138_diag | 扁平 `[int; 4]` 2x2 对角和（无嵌套字面量） |
| 73_area | 133_area | 结构体作函数参数（`6x7=42`） |
| 74_manhattan | 151_obj_param | 对象参数坐标求和（与 73 互补） |
| 75_weekend | 152_enum_calc | `enum + match` 周末判断（全臂列举） |
| 76_flags | 106_flags | `|` 组合 + `&` 测试 + `^` 翻转（与 44 互补） |
| 77_find_index | 108_find_index | 显式循环 + 闭包谓词表达 `findIndex` |
| 78_map_filter | 114_compose | 两轮显式循环表达 `map/filter` 链 |
| 79_nested | 33_nested_struct | 整数嵌套结构体读值（字符串字段暂略） |
| 80_gcd_all | 181_gcd_all | 循环版多组 `gcd` 打印（与 17/21/51 互补） |
| 81_reduce | 25_reduce | 显式循环表达 `reduce` 求和 + 右折求积 |
| 82_reduce_right | 107_reduce_right | 右折减法（`0-4-3-2-1=-10`，与 81 互补） |
| 83_slice_concat | 27_slice_concat | `Vec::push` 手工表达 `slice/concat` |
| 84_spread | 29_spread_elem | 拷贝循环 + `push` 表达展开（无展开语法） |
| 85_rest | 154_rest_first | 定长数组形参表达剩余参数（无 `...rest`） |
| 86_destructure | 156_destructure_call | 显式索引表达解构（无解构语法） |
| 87_chainwrite | 113_chainwrite | 嵌套结构体字段写（与 79 互补） |
| 88_grade | 23_switch_plain | 纯全 `return` 臂分档（#7 已修复） |
| 89_splice | 166_splice_do | 开区间尾段手工切片（与 65/83 同口径） |
| 90_f64_cmp | 22_f64_cmp | 浮点运算比较（首个 f64 demo） |
| 91_lcm_all | 182_lcm_all | 循环版 `gcd` + `lcm` 多组打印（与 21 互补） |
| 92_and_or | 197_and_or | 整数比较式改写与或非（`&&` 口径见 14） |
| 93_override | 104_override | 双结构体各自实现同名方法（无继承语义） |
| 94_nested_tpl | 105_nested_tpl | 双插值模板赋值打印 + `str_eq`（#4b 绕行） |
| 95_str_iter | 11_string_array_iter | 直列 `len()` 改写 `forEach/map`（无迭代器协议） |
| 96_forof_len | 12_for_of | `for-of` 改直列求长（无迭代器协议） |
| 97_gcd_loop | 134_gcd_loop | `main` 内直写辗转相除（与 51/80 互补） |
| 98_sort_copy | 26_sort | 原地排序 + 手工拷贝排序（无 `toSorted`） |
| 99_closure_dbl | 30_closure | 最简闭包绑定调用（与 20 互补） |
| 100_math_floor | 31_math | 分支 `abs` + `as i32` 截断 `floor` |
| 101_map_getset | 35_map_getset | 缺键取零表 `has` + `len(m)` 表长（#14 已修复） |
| 102_map_count | 132_map_count | 取-加-存覆盖累加（11, 2） |
| 103_vec_ops | 28_push_unshift | 整数核手写 `unshift/fill/with`（无方法） |
| 104_generic | 34_generic_fn | `int + bool` 两具化（字符串实例暂略） |
| 105_map_clear | 167_map_clear | `clear()` 后表长为 0（`MAP_CLEAR` 直降） |
| 106_postfix | 268_postfix | 显式加减表达后置自增（无 `++`/`--`） |
| 107_switch_stacked | 282_switch_stacked | 胖箭头两臂同体（无 C 式堆叠 `case`） |
| 108_f64_loop | 279_f64_loop | `while` 显式累加 f64 步进（`n=3,m=7`） |
| 109_f64_neg | 313_fneg | f64 取负 + 比较（`11`） |
| 110_f64_array | 315_float_arr | f64 数组局部存取（#17 绕行：不跨函数返回） |
| 111_struct_args | 310_iface_lit_arg | 结构体字面量/绑定作实参（无继承） |
| 112_vec_grow | 269_arr_rebind | `Vec::push` 表达数组生长（无 `concat`） |
| 113_pow_loop | 273_pow_assign | 循环连乘表达幂赋值（无 `**`/`**=`） |
| 114_struct_pair | 272_destructure_defaults | 显式字段/索引表达解构（无解构语法） |
| 115_switch_dispatch | 271_switch_arms | switch 分发 + `else if` 链（`12,100`） |
| 116_linear_search | 323_unannot_scan | 显式循环表达 indexOf/includes/lastIndexOf（#1 注解） |
| 117_isqrt | 347_math_sqrt | 循环试乘整数开方下取整（无 `Math.sqrt`） |
| 118_sign_imul | 351_math_small | 分支 `sign` + 普通乘法 `imul`（`abs` 见 100） |
| 119_shift | 372_ushr + 412_shift_compound | `>>`/`<<` + 显式移位赋值（无 `>>>`/`<<=`） |
| 120_recurse | 377_funexpr_rec | 具名函数递归 fact/fib（无函数表达式自引用） |
| 121_vec_rebind | 381_arr_clear | 重建绑定表达清空（无 `Vec.clear`） |
| 122_strcmp_lit | 384_strcmp | 字面量关系比较 + helper 包裹（#18 绕行） |
| 123_inbounds | 387_inarr | 显式区间判断表达 `in`（无 `in` 运算符） |
| 124_bool_array | 388_boolkw | 布尔透传 + `bool` 数组 |
| 125_f64_infer | 311_float_infer | 无注解浮点即 `f64` |
| 126_str_switch | 498_str_switch | 字面量 scrutinee + 字面量臂（#18 绕行） |
| 127_arr_swap | 547_swap | 数组形参调用者可见可变（引用语义） |
| 128_shift_big | 452_shift_bigcount | 64 位无掩码位移（与 TS mod-32 不同） |
| 129_truthiness | 442_boolean_ctor | 显式 `!= 0` 表达真值（条件须严格布尔） |
| 130_floor_neg | 497_math_floor_neg + 545_f64round | 分支 floor/ceil/trunc（主覆盖负数） |
| 131_arr_alloc | 446_new_array_n | 占位字面量 + 下标填充（无 `new Array(n)`） |
| 132_struct_str | 552_strobj | `: ptr` 字符串字段 + `len`/`str_eq` |
| 133_pi_const | 574_piconst | 顶层 `const` f64 比较 |
| 134_max_min | 511_math_int | 分支 max/min 双函数（无 `Math.max/min`） |
| 135_enum_ret | 575_enumret | 函数返回枚举 + `match` 消费 |
| 136_top_const_arr | 698_top_const_arr | 顶层 `const` 数组跨函数读（结构体常量见 #19） |
| 137_oob_zero | 701_top_arr_oob | 动态越界读零（字面量越界 exe 不支持） |
| 138_eager_logic | 643_logic_short | `&&`/`||` 急求值（与短路语义不同，见 #21） |
| 139_discard_loop | 672_void_i32 + 676_void_loop | 弃值调用（无 `void` 运算符） |
| 140_indexof_from | 710_top_arr_indexof | 起始位查找（116 延伸） |
| 141_vec_pop | 507_arr_api | `pop().unwrap()`（空表 `None`） |
| 142_prime_count | 183_prime_upto | 试除法素数计数（与 18 筛法互补） |
| 143_fib_iter | 173_fib_loop | 迭代 fib（与 120 递归互补） |
| 144_struct_bool | 790_top_obj_bool | 局部布尔字段结构体（顶层常量见 #19） |
| 145_hex_mask | 1250_mask_idioms | `0x` 字面量掩码（无 `0o`/`0b`；`&` 优先级坑） |
| 146_nested_continue | 893_nested_continue | 嵌套循环 continue（label 跳出见 #9） |
| 147_while_continue | 895_while_continue | while + continue 求和（先步进防死循环） |
| 148_call_len | 898_chain_call_len | `: ptr` 返回再 `len`（未注解落 #15） |
| 149_multi_enum | 842_top_multi_enum | 显式判别值 + 变量 scrutinee |
| 150_idx_compound | 908_arr_compound + 940_compound_var_idx | 下标复合赋值（字面量 + 变量下标） |
| 151_switch_call | 902_switch_call | 调用结果作 switch scrutinee |
| 152_enum_switch | 943_enum_switch | 枚举 switch（与 match 互补） |
| 153_case_const | 948_case_ident | 顶层 `const` 作 case 臂 |
| 154_vec_bool | 388_boolkw | `Vec<bool>`（124 定长版的换 Vec 版） |
| 155_neg_divmod | 383_div_guard | 负数除余向零截断（383 正数延伸） |
| 156_copywithin | 981_copywithin | 显式循环前向重叠拷贝（无方法） |
| 157_ilog10 | 983_math_aux | 循环除数 log10（`sign` 见 118） |
| 158_step3_down | 1048_for_dec | 步长 3 倒数求和（`for` 无步长） |
| 159_while_dec2 | 1050_while_dec | 步长 2 倒数求和（与 158 互补） |
| 160_rev_index_sum | 1052_rev_index | 倒序下标求和（`len()` 先 `as i32`） |
| 161_deep_break | 1053_deep_break | 三层嵌套裸 break（label 见 #9） |
| 162_switch_in_loop | 1055_sw_loop | 循环内 switch（偶 +10/奇 +1） |
| 163_call_bound_loop | 1057_for_call_bound | 调用作循环上界（逐次重求） |
| 164_closure_array | 1071_closure_arr | 捕获数组的闭包（体仅表达式） |
| 165_bool_param | 1044_bool_param | 布尔参数取反（严格布尔） |
| 166_switch_bool | 1099_switch_bool | 布尔字面量臂（条件臂见 #22） |
| 167_arr_identity | 1202_arr_identity + 1203_self_identity | 数组 `==` 为引用恒等 |
| 168_shift_32bit | 1228_shift_sign | 32 位边界 64 位语义（无 `>>>`） |
| 169_bool_not | 1220_double_negation | `!`/`!!` 布尔操作数 |
| 170_shift_loop | 1234_shift_loop | 循环左移翻倍（与 52 机制互补） |
| 171_overflow_64 | 1237_mul_overflow | 64 位不回绕（TS 末项分歧） |
| 172_bit_test | 1248_bit_condition + 1249_bit_ternary | 位→布尔判定（显式比零） |
| 173_xor_swap | 1251_xor_swap | 三遍 `^` 交换（无 `^=`） |
| 174_clamp | 1263_clamp | 三参数夹逼（与 134 互补） |
| 175_between | 1267_between | `&&` 双边区间判定 |
| 176_shift_varcount | 1233_shift_varcount | 变量位移计数（119 字面量版互补） |
| 177_shift_chain | 1239_shift_chain + 1247_shift_compare | 连续位移 + 符号判定（64 位） |
| 178_shift_halve | 1256_shift_while | 循环右移折半（与 170 方向互补） |
| 179_compare_chain | 1258_compare_chain | 比较传递链（与 175 同构） |
| 180_swbind_str | 435_str_switch | `: ptr` 绑定 scrutinee（#18 缩小） |
| 181_find_last | 430_find_last | 倒序查找（无 `findLast`） |
| 182_struct_value | 476_field_compound | 结构体形参值语义（对照 127） |
| 183_factory_struct | 482_factory_arg | 工厂返回结构体（对照 #17） |
| 184_paren_call | 481_paren_call | 括号包裹被调函数 |
| 185_nested_ternary | 1262_nested_ternary | 右嵌套三元分档（06 互补） |
| 186_shift_varcount | 1233_shift_varcount | 变量位移计数（局部 `const` 不可用） |
| 187_two_structs | 791_top_obj_multi | 双局部结构体交叉读（顶层见 #19） |
| 188_f64_pick | 475_f64_mixed_ternary | f64 三元钳零（185 整数版互补） |
| 189_calls_array | 1063_calls_in_array | 调用结果组数组 |
| 190_generic_infer | 1075_generic_erased | 泛型推断调用（104 显式版互补） |
| 191_btree_basic | 132_map_count | 首个 BTreeMap 端到端（#14 口径） |
| 192_result_basic | 1026_catch_arith | 首个 Result demo（无 try/catch） |
| 193_str_array | 286_str_elem | 定长字符串数组存取（无 `split`） |
| 194_f64_param | 1011_arr_param_twice | f64 数组形参（变长不可表达） |
| 195_nest3 | 916_nested_struct | 三层嵌套穿透读（79 两层互补） |
| 196_nested_count | 1278_nested_loop | 裸双层循环计数（146 分支版互补） |
| 197_step2_up | 1279_while_even | 步长 +2 上行累加（159 下行互补） |
| 198_clz32 | 1438_clz32 | 手写前导零计数（178 右移同机制） |
| 199_dowhile_continue | 1395_dowhile_continue | do-while 语义 + 跳过（40 无跳过互补） |
| 200_loop_carry | 1397_loop_var_after | 循环外带末轮值（163 调用界互补） |
| 201_pop_drain | 1416_pop_cond | `len` 驱动排空求和（141 单 pop 互补） |
| 202_for_break | 1270_for_break | 单层区间 for 早停（32 while 版互补） |
| 203_prefix_slice | 1423_slice_0_2 | 手写前缀切片（89 尾段版互补） |
| 204_cond_break | 1268_while_break | 条件循环跳过加早停（147 无停互补） |
| 205_ceil_neg | 1426_ceil_neg | 负数上取整（130 未测分支） |
| 206_iife_nest | 864_top_iife_nest | 嵌套直接调用（57 单层互补） |
| 207_tern_switch | 942_ternary_switch | 三元判别 switch（151 调用版互补） |
| 208_void_fn | 915_void_call | 首个 void 函数（668 同形） |
| 209_boolret | 567_boolret | 布尔返回函数（165 形参互补） |
| 210_sw_field | 939_sw_member | 字段路径 switch（151/207 互补） |
| 211_void_nest | 671_void_fn | void 嵌套调用（208 单层互补） |
| 212_discard_call | 672_void_i32 | 返回值丢弃（141 语句版互补） |
| 213_void_branch | 675_void_branch | 分支内 void 调用 |
| 214_void_loop | 676_void_loop | 循环内 void 调用 |
| 215_dyn_range | 605_top_cond_forlen | 动态上界区间（此前皆字面量） |
| 216_f64_channel | 278_f64_channel | f64 通道（125 比较口径） |
| 217_intmap | 35_map_getset | int 键版（101 字符串版互补） |
| 218_nested_calls | 1062_nested_calls | 实参位嵌套（189 组数组互补） |
| 219_chainret | 1065_chained_returns | 返回表达式组合 |
| 220_elseif_calls | 1066_elseif_calls | 调用判别链（50 变量版互补） |
| 221_gen_struct | 343_generic_erase | 泛型结构体（104 函数版互补） |
| 222_btree_overwrite | 132_map_count | BTree 覆写（102 HashMap 版互补） |
| 223_vec_struct | 13_struct | 结构体数组（141×11 组合） |
| 224_nest4 | 916_nested_struct | 四层穿透（195 三层延伸） |
| 225_struct_write | 13_struct | 单层字段写（87 嵌套版互补） |
| 226_idx_copy | 79_copy_loop | 下标逐元拷贝（203 push 版互补） |
| 227_avg | 76_sum_avg | 整平均（27+45 组合） |
| 228_for_continue | 38_nested_loops | 区间嵌套跳过（146 while 版互补） |
| 229_nest_vec | 172_sum_2d | 嵌套 Vec（58 扁平版互补） |
| 230_fact_iter | 125_fact | 迭代阶乘（120 递归版互补） |
| 231_minmax_sum | 1264_minmax_sum | 极值组合（134 单体互补） |
| 232_leap_list | 25_leap | 闰年表（25 单点互补） |
| 233_fizz_count | 24_fizzbuzz | 整除计数（24 打印版互补） |
| 234_numpalin | 103_palindrome | 算术回文（30 数组版互补） |
| 235_sw_arm | 271_switch_arms | 臂内计算（107 常量版互补） |
| 236_collatz_seq | 155_collatz | 序列版（49 步数版互补） |
| 237_fib_list | 173_fib_loop | 表版（143 单值版互补） |
| 238_prime_list | 183_prime_upto | 表版（46 计数版互补） |
| 239_f64_arith | 278_f64_channel | 减乘除（278 加法互补） |
| 240_vec_dot | 142_dot | Vec 版（27 定长版互补） |
| 241_sieve_mark | 128_sieve | 真筛版（18 试除版互补） |
| 242_minmax_fn | 1264_minmax_sum | 三元体版（134 分支版互补） |
| 243_cumsum | 141_sum_sq | 累计版（27 总额版互补） |
| 244_prefix_max | 121_second_max | 前缀版（22 单遍版互补） |
| 245_formula | 137_tri | 公式版（71/27 循环版互补） |
| 246_vec_reverse | 1441_toReversed | 手写逆序（89 尾段版互补） |
| 247_vec_rotate | 122_rotate | 循环版（23 展开版互补） |
| 248_vec_write | 1281_arr_index | Vec 版（226 定长版互补） |
| 249_vec_swap | 547_swap | Vec 版（127 定长版互补） |
| 250_vec_max | 134_max_min | 扫描版（134 函数版互补） |
| 251_vec_min | 134_max_min | 镜像版（250 互补） |
| 252_vec_prod | 125_fact | Vec 版（230 区间版互补） |
| 253_vec_equal | 167_arr_identity | 逐元版（167 引用版互补） |
| 254_run_prod | 243_cumsum | 积版（243 和版互补） |
| 255_vec_second | 121_second_max | Vec 版（22 定长版互补） |
| 256_gcd_sub | 119_gcd3 | 减法版（17/80 取模版互补） |
| 257_isqrt_newton | 347_math_sqrt | 牛顿版（117 试乘版互补） |
| 258_pow_sqmul | 1231_pow | 平方乘版（49/113 连乘版互补） |
| 259_divmod_sub | 179_div_mod | 减法版（45 算子版互补） |
| 260_bsearch_rec | 101_bsearch | 递归版（17 循环版互补） |
| 261_prime_wheel | 183_prime_upto | 轮式版（46 试除版互补） |
| 262_select_sort | 26_sort | 选择版（16/38/98 互补） |
| 263_lower_bound | 101_bsearch | 下界版（17 等值版互补） |
| 264_mult_table | 893_nested_continue | 嵌套算术（146 计数互补） |
| 265_deep_count | 38_nested_loops | 三层版（228 双层互补） |
| 266_unwrap_or | 1022_map_str_int | `-or` 版（101 取零版互补） |
| 267_result_or | 1026_catch_arith | `-or` 版（192 基础版互补） |
| 268_hashset_int | 1104_set_has_size | 首个 HashSet（#16 互补） |
| 269_btreeset_str | 1321_set_add_add | 首个 BTreeSet（#16 互补） |
| 270_deep_write | 87_chainwrite | 三层版（87 双层互补） |
| 271_vec_struct_write | 223_vec_struct | 写版（223 读版互补） |
| 272_vec_enum | 149_multi_enum | 容器版（149 变量版互补） |
| 273_hashset_str | 1104_set_has_size | str 版（268 互补） |
| 274_opt_pred | 15_option | 谓词版（15 match 版互补） |
| 275_fixed_dynrange | 605_top_cond_forlen | 定长版（215 Vec 版互补） |
| 276_method_args | 14_class | 带参版（12 无参版互补） |
| 277_fact_list | 125_fact | 表版（230 单值版互补） |
| 278_sq_list | 141_sum_sq | 表版（27 总额版互补） |
| 279_tri_list | 137_tri | 表版（71 单值版互补） |
| 280_pow_list | 176_pow_sum | 表版（62 求和版互补） |
| 281_prime_sum | 183_prime_upto | 求和版（238 表版互补） |
| 282_fib_sum | 173_fib_loop | 求和版（237 表版互补） |
| 283_collatz_max | 155_collatz | 最值版（236 序列版互补） |
| 284_odd_sum | 04_while_sum | 步进版（04 逐一版互补） |
| 285_fact_sum | 125_fact | 求和版（230 单值版互补） |
| 286_duel_gcd | 17×256 | 取模减法对峙（6=6） |
| 287_duel_isqrt | 117×257 | 试乘牛顿对峙（3=3） |
| 288_duel_sqsum | 27×245 | 循环公式对峙（55=55） |
| 289_duel_sort | 16×262 | 插入选择对峙（1/5/3） |
| 290_duel_prime | 46×261 | 试除轮式对峙（10=10） |
| 291_duel_fib | 120×143 | 递归迭代对峙（55=55） |
| 292_duel_fact | 120×230 | 递归迭代对峙（120=120） |
| 293_duel_pow | 113×258 | 连乘方乘对峙（1024） |
| 294_duel_collatz | 49×236 | 步数链长对峙（8=8） |
| 295_duel_tri | 71×245 | 循环公式对峙（55=55） |
| 296_duel_abs | 100×06 | 语句表达式对峙（5=5） |
| 297_duel_min | 134×242 | 语句三元对峙（3=3） |
| 298_duel_avg | 227×245 | 循环公式对峙（5=5） |
| 299_duel_max | 134×242 | 语句三元对峙（7=7） |
| 300_duel_swap | 127×173 | 临时异或对峙（34,12） |
| 301_duel_rotid | 247×247 | 旋转复原对峙（1,5） |
| 302_duel_contains | 39×116 | 谓词下标对峙（1/0） |
| 303_duel_second | 22×255 | 定长 Vec 对峙（9=9） |
| 304_duel_pick | 03×06 | 语句三元对峙（7=7） |
| 305_duel_sum | 04×05 | while for 对峙（55=55） |
| 306_bubble_sort | 73_bubble_sort | 第三种手排（1,3,5） |
| 307_tri_fact | 120×230×277 | 三向对峙（120 三版） |
| 308_tri_fib | 120×143×237 | 三向对峙（55 三版） |
| 309_tri_pow | 113×258×119 | 三向对峙（1024 三版） |
| 310_gcd_stein | 119_gcd3 | Stein 第三实现（6,1） |
| 311_tri_tri | 71×245×rec | 三向对峙（55 三版） |
| 312_duel_min3 | 134×185 | 分支三元对峙（1=1） |
| 313_tri_search | 116×17×39 | 三向对峙（2=2=1） |
| 314_duel_stats | 22×27 | 单遍三遍对峙（35=35） |
| 315_tri_sum | 61×305×245 | 三向对峙（55 三版） |
| 316_tri_sort | 16×262×306 | 三向对峙（3 三版） |
| 317_sqrt_bs | 347_math_sqrt | 二分版（117/257 互补） |
| 318_cbrt | 347_math_sqrt | 立方版（117 平方版互补） |
| 319_tri_prime | 46×261×241 | 三向对峙（10 三版） |
| 320_ilog2 | 983_math_aux | log2 部（157 互补） |
| 321_is_square | 347_math_sqrt | 判定版（117 取值版互补） |
| 322_duel_clamp | 26×174 | 语句三元对峙（5=5） |
| 323_tri_gcd | 17×256×310 | 三向对峙（25 三版） |
| 324_duel_rev | 246×246 | 自对偶复原（301 异操作互补） |
| 325_is_tri | 137_tri | 成员版（71 取值版互补） |
| 336_euler_phi | 183_prime_upto | 计数版（46 素数版互补） |
| 337_div_count | 183_prime_upto | 计数版（334 和版互补） |
| 338_pyth_check | 347_math_sqrt | 判定应用（321 互补） |
| 339_pyth_gen | 117_isqrt | 生成版（338 判定互补） |
| 340_fib_gcd | 120×17 | 恒等式对峙（2=2） |
| 341_wilson | 230×328 | 定理合取（5→1） |
| 342_droot | 103_palindrome | 数位版（234 回文版互补） |
| 343_pal_count | 234_numpalin | 计数版（234 单值版互补） |
| 344_modinv | 181_gcd_all | 逆元版（80 互补） |
| 345_euler_thm | 328×336 | 定理合取（1） |
| 326_mat_det | 143_mat_add | 行列式版（19 加法版互补） |
| 327_mat_trace | 143_mat_add | 迹版（19 加法版互补） |
| 328_powmod | 258_pow_sqmul | 取模版（258 明文版互补） |
| 329_quad_roots | 117_isqrt | 求根应用（117 取值版互补） |
| 330_arith_seq | 04_while_sum | 等差版（04 逐一版互补） |
| 331_narci | 103_palindrome | 立方和版（234 回文版互补） |
| 332_palprime | 234×238 | 回文素数合取（234/46 互补） |
| 333_is_fib | 173_fib_loop | 成员版（237 表版互补） |
| 334_perfect | 183_prime_upto | 因子和版（46 素数版互补） |
| 335_amicable | 334_perfect | 亲和版（334 互补） |
| 346_coprime_pairs | 336_euler_phi | 判定版（336 计数版互补） |
| 347_gcd_chain | 340_fib_gcd | 链式版（80/119 口径） |
| 348_lcm_chain | 21_gcd_lcm | 链式版（91 互补） |
| 349_fact_zero | 125_fact | 数位版（285 求和版互补） |
| 350_binom | 137_tri | 组合版（325 成员版互补） |
| 351_catalan | 350_binom | 商式版（350 互补） |
| 352_geom_sum | 330_arith_seq | 等比版（330 等差版互补） |
| 353_mat_pow | 28_mat_mul | 幂版（326/327 互补） |
| 354_mat_trans_sum | 19_matrix | 恒等式版（327 迹版互补） |
| 355_tri_powmod | 328_powmod | 三向版（258 明文版互补） |
| 356_is_prime | 46_trial_div | 判定版（261 轮式版互补） |
| 357_nth_prime | 356_is_prime | 计数版（356 判定版互补） |
| 358_prime_sum | 281_prime_sum | 素数版（238 表版互补） |
| 359_sq_sum | 27_sum_sq | 循环版（288 公式版互补，365 做三向） |
| 360_cube_sum | 359_sq_sum | 立方版（359 平方版互补） |
| 361_fib_even_sum | 237_fib_table | 偶项版（282 求和版互补） |
| 362_harm_bound | 141_sum | 倒数版（239 f64 版互补，纯 int） |
| 363_gcd_table_sum | 125_table | gcd 版（173-表系互补） |
| 364_duel_prime | 46×261 | 判定对峙版（290 互补） |
| 365_tri_sqsum | 288_duel_sqsum | 三向版（359 互补） |
| 366_twin_count | 356_is_prime | 对版（358 求和版互补） |
| 367_prime_gap | 358_prime_sum | 间隙版（357 计数版互补） |
| 368_tri_cubesum | 365_tri_sqsum | 立方版（360 互补） |
| 369_alt_sum | 359_sq_sum | 符号版（330 等差系互补，375 做三向） |
| 370_gcd_grid_sum | 363_gcd_table_sum | 网格版（363 单表版互补） |
| 371_lcm_table_sum | 348_lcm_chain | 表版（363-gcd 表互补） |
| 372_pow3_sum | 352_geom_sum | 3 进制版（352 公比 2 版互补） |
| 373_prime_count | 358_prime_sum | 计数版（357 单值版互补） |
| 374_duel_primecount | 364_duel_prime | 计数对峙版（373 互补） |
| 375_tri_altsum | 368_tri_cubesum | 交错版（369 互补） |
| 376_semiprime | 356_is_prime | 合数版（356 素数版互补） |
| 377_sigma | 337_div_count | 求和版（337 计数版互补，379 做三向） |
| 378_abundant | 377_sigma | 分类版（334 完全数版互补） |
| 379_tri_sigma | 368_tri_cubesum | 除数和版（377 互补） |
| 380_goldbach | 366_twin_count | 和版（366 对版互补） |
| 381_coprime_sum | 346_coprime_pairs | 求和版（336-phi 计数版互补） |
| 382_phi_sum | 336_euler_phi | 累和版（381 互补） |
| 383_lcm_grid | 370_gcd_grid_sum | lcm 版（371 单表版互补） |
| 384_duel_sigma | 374_duel_primecount | 除数和版（379 三向版互补） |
| 385_tri_pow3sum | 375_tri_altsum | 等比版（372 互补） |
| 386_omega | 376_semiprime | 去重版（376 计重版互补） |
| 387_mobius | 386_omega | 符号版（386 计数版互补） |
| 388_primepow | 376_semiprime | 单基版（376 多基版互补） |
| 389_perfect | 378_abundant | 等值版（334 因子和版互补） |
| 390_amicable_pair | 335_amicable | 函数版（335 互补） |
| 391_mersenne | 356_is_prime | 指数版（358 计数版互补） |
| 392_fact_prime_exp | 349_fact_zero | 素数版（349 零计数版互补） |
| 393_binom_odd | 350_binom | 奇偶版（350 取值版互补） |
| 394_duel_catalan | 351_catalan | 递推对峙版（351 商式版互补） |
| 395_tri_coprime | 381_coprime_sum | 三向版（381 互补） |
| 396_squarefree | 387_mobius | 存在版（387 零值版互补） |
| 397_fermat | 356_is_prime | 概率版（341 伪素数记档） |
| 398_wilson | 397_fermat | 阶乘版（397 幂模版互补） |
| 399_phi_mult | 336_euler_phi | 乘性版（382 累和版互补） |
| 400_sigma_mult | 377_sigma | 乘性版（399-phi 乘性互补） |
| 401_lcm_gcd_id | 348_lcm_chain | 恒等式版（348 链式版互补） |
| 402_units_prod | 381_coprime_sum | 乘积版（381 求和版互补） |
| 403_duel_fermat | 355_tri_powmod | 费马版（397 互补） |
| 404_tri_phi | 395_tri_coprime | phi 版（399 互补） |
| 405_duel_euler | 403_duel_fermat | 二次剩余版（397 互补） |
| 406_dsum | 342_droot | 求和版（342 数位根版互补，411/414 做对峙三向） |
| 407_dprod | 406_dsum | 乘积版（406 求和版互补） |
| 408_persist | 406_dsum | 迭代版（406 单步版互补） |
| 409_rev_num | 234_numpalin | 反转版（234 判定版互补，412 做对峙） |
| 410_harshad | 406_dsum | 整除版（406 求和版互补，415 做对峙） |
| 411_duel_dsum | 364_duel_prime | 数位版（406 互补） |
| 412_duel_rev | 411_duel_dsum | 反转版（409 互补） |
| 413_repunit | 352_geom_sum | 1 串版（352 公比版互补） |
| 414_tri_dsum | 411_duel_dsum | 三向版（408-持久性互补） |
| 415_duel_harshad | 411_duel_dsum | 整除版（410 互补） |
| 416_sphenic | 386_omega | 等三版（386 计数版互补） |
| 417_smooth | 416_sphenic | 界版（416 形态版互补，425 做三向） |
| 418_weak_goldbach | 380_goldbach | 奇三元版（380 强分拆互补） |
| 419_sophie | 366_twin_count | 倍加版（366 对版互补） |
| 420_cunningham | 419_sophie | 链版（419 单步版互补） |
| 421_mertens | 387_mobius | 累和版（387 单值版互补，424 做对峙） |
| 422_liouville | 421_mertens | 全计数版（421 无平方版互补） |
| 423_duel_sphenic | 394_duel_catalan | 形态版（416 互补） |
| 424_duel_mertens | 423_duel_sphenic | 求和版（421 互补） |
| 425_tri_smooth | 404_tri_phi | 形态版（417 互补） |
| 426_prim_pyth | 338_pyth_check | 计数版（338/339 互补） |
| 427_euclid_gen | 339_pyth_gen | 公式版（339 枚举版互补） |
| 428_taxicab | 426_prim_pyth | 立方版（426 平方版互补） |
| 429_collatz_peak | 236_collatz | 峰值版（283 最值版互补，432 做对峙） |
| 430_prime_quad | 366_twin_count | 四元版（366 对版互补，434 做对峙） |
| 431_prime_triplet | 430_prime_quad | 三元版（430 四元版互补） |
| 432_duel_peak | 411_duel_dsum | 冰雹版（429 互补） |
| 433_sexy_primes | 366_twin_count | 差六版（366 差二版互补） |
| 434_duel_quad | 423_duel_sphenic | 素串版（430 互补） |
| 435_tri_constellation | 431_prime_triplet | 三星座版（366/430/431 合取） |
| 436_egcd | 347_gcd_chain | 系数版（347 取值版互补） |
| 437_crt | 344_modinv | 应用版（344 逆元版互补） |
| 438_absorb | 401_lcm_gcd_id | 格版（401 恒等式版互补） |
| 439_fibmod | 237_fib_table | 取模版（237 明文版互补，444/445 做对峙三向） |
| 440_powmod_sum | 352_geom_sum | 取模版（352 明文版互补） |
| 441_euclid_steps | 347_gcd_chain | 步数版（347 取值版互补） |
| 442_lucas | 237_fib_table | 伴随版（237 互补） |
| 443_pell | 442_lucas | 二倍递推版（442 一倍版互补） |
| 444_duel_fibmod | 364_duel_prime | 周期版（439 互补） |
| 445_tri_fibmod | 444_duel_fibmod | 三向版（439 互补） |
| 446_jacobsthal | 442_lucas | 加权版（442 一倍版互补） |
| 447_pell_comp | 443_pell | 伴随版（443 本体版互补） |
| 448_tribonacci | 237_fib_table | 三阶版（237 二阶版互补，453 做对峙） |
| 449_padovan | 448_tribonacci | 跳阶版（448 全加版互补） |
| 450_perrin | 449_padovan | 异初值版（449 初值版互补） |
| 451_bell | 351_catalan | 集合划分版（351 互补，454 做对峙） |
| 452_eulerian | 451_bell | 排列版（451 组合版互补，455 做对峙） |
| 453_duel_trib | 432_duel_peak | 三阶版（448 互补） |
| 454_duel_bell | 453_duel_trib | 集合版（451 互补） |
| 455_duel_eulerian | 454_duel_bell | 排列版（452 互补） |
| 456_cows | 449_padovan | 同构版（449 跳阶版互补） |
| 457_sylvester | 446_jacobsthal | 乘积递推版（446 加权版互补） |
| 458_lazy | 330_arith_seq | 二次版（330 一次版互补，460 做对峙） |
| 459_cake | 458_lazy | 三次版（458 二次版互补，463 做对峙） |
| 460_duel_lazy | 288_duel_sqsum | 切分版（458 互补） |
| 461_delannoy | 360_cube_sum | 格路版（19-矩阵系互补，464/465 做对峙三向） |
| 462_motzkin | 351_catalan | 不交弦版（351 全分拆版互补） |
| 463_duel_cake | 460_duel_lazy | 三次版（459 互补） |
| 464_duel_delannoy | 463_duel_cake | 格路版（461 互补） |
| 465_tri_delannoy | 464_duel_delannoy | 三向版（461 互补） |
| 466_pentagonal | 137_tri | 高边版（137 三角版互补，473 做对峙） |
| 467_hexagonal | 466_pentagonal | 六边版（466 五边版互补） |
| 468_heptagonal | 467_hexagonal | 七边版（467 六边版互补） |
| 469_octagonal | 468_heptagonal | 八边版（468 七边版互补） |
| 470_tetrahedral | 137_tri | 堆叠版（137 平面版互补） |
| 471_pyramid | 359_sq_sum | 闭式版（359 循环版互补） |
| 472_star | 466_pentagonal | 星形版（466 多边版互补，474/475 做对峙三向） |
| 473_duel_pent | 460_duel_lazy | 多边形版（466 互补） |
| 474_duel_star | 473_duel_pent | 星形版（472 互补） |
| 475_tri_star | 474_duel_star | 三向版（472 互补） |
| 476_egypt | 362_harm_bound | 单位分数版（362 缩放版互补，481 做三向） |
| 477_sylv_egypt | 457_sylvester | 倒数版（457 本体版互补） |
| 478_farey_len | 373_prime_count | 互素版（373 素数版互补，482 做对峙） |
| 479_mediant | 478_farey_len | 构造版（478 长度版互补） |
| 480_harm_frac | 362_harm_bound | 既约版（362 缩放版互补） |
| 481_tri_egypt | 476_egypt | 三向版（476 双角版互补） |
| 482_duel_farey | 374_duel_primecount | 分数版（478 互补） |
| 483_stern | 237_fib_table | 位折半版（237 线性版互补） |
| 484_carmichael | 336_euler_phi | 指数版（336/399 互补） |
| 485_primroot | 397_fermat | 阶版（397 判定版互补） |
| 486_heron | 338_pyth_check | 面积版（338 判定版互补，490 做对峙） |
| 487_brahmagupta | 486_heron | 四边版（486 三边版互补） |
| 488_pell_eq | 329_quad_roots | 丢番图版（329 求根版互补，494 做对峙） |
| 489_markov | 428_taxicab | 三元版（428 二元版互补，492 做三向） |
| 490_duel_heron | 434_duel_quad | 几何版（486 互补） |
| 491_bernoulli | 480_harm_frac | 递推版（480 求和版互补） |
| 492_tri_markov | 489_markov | 三向版（489 双角版互补） |
| 493_mihailescu | 397_fermat | 幂差版（397 伪素数版互补） |
| 494_duel_pelleq | 403_duel_fermat | 搜索版（488 互补） |
| 495_genocchi | 491_bernoulli | 组合版（491 本体版互补） |
| 496_contfrac | 476_egypt | 欧几里得版（476 贪心版互补，497 做收敛子） |
| 497_convergent | 496_contfrac | 渐近版（496 展开版互补） |
| 498_pell_gen | 488_pell_eq | 幂版（488 基本解版互补） |
| 499_legendre | 405_duel_euler | 符号版（405 对峙版互补，504/505 做对峙三向） |
| 500_jacobi | 499_legendre | 合数版（499 素数版互补） |
| 501_qr_count | 499_legendre | 计数版（499 判定版互补） |
| 502_primroot_count | 485_primroot | 计数版（485 最小值版互补） |
| 503_dlog | 485_primroot | 逆问题版（485 求根版互补） |
| 504_duel_legendre | 434_duel_quad | 剩余版（499 互补） |
| 505_tri_legendre | 435_tri_constellation | 剩余版（499/504 合取） |
| 506_zeckendorf | 237_fib_table | 贪心版（237 迭代版互补，507 做编码） |
| 507_fibcode | 506_zeckendorf | 编码版（506 项数版互补） |
| 508_bitcount | 406_dsum | 二进制版（406 十进制版互补，510/512 做对峙三向） |
| 509_bitrev | 409_rev_num | 位版（409 十进制版互补） |
| 510_duel_bitcount | 411_duel_dsum | 位版（508 互补） |
| 511_hamming | 510_duel_bitcount | 应用版（510 计数版互补） |
| 512_tri_bitcount | 510_duel_bitcount | 三向版（508 互补） |
| 513_nibble_swap | 509_bitrev | 半字节版（509 全字节版互补） |
| 514_parity | 508_bitcount | 奇偶版（508 计数版互补） |
| 515_pow2check | 514_parity | 位技巧版（514 计数版互补） |
| 516_golomb | 448_tribonacci | 自指版（448 线性版互补，522 做对峙） |
| 517_ruler | 392_fact_prime_exp | 单值版（392 累计版互补） |
| 518_aliquot_steps | 378_abundant | 链版（378/389 互补） |
| 519_deficient_count | 378_abundant | 亏版（378 丰版互补，521 做三向） |
| 520_duel_aliquot | 384_duel_sigma | 真因子版（390 互补） |
| 521_tri_classify | 435_tri_constellation | 分类版（378/389/519 合取） |
| 522_duel_golomb | 453_duel_trib | 自指版（516 互补） |
| 523_perfect_count | 373_prime_count | 完全版（373 素数版互补） |
| 524_abundant_sum | 358_prime_sum | 丰数版（358 素数版互补） |
| 525_practical | 378_abundant | 划分版（378 盈亏版互补） |
| 526_distinct | 525_practical | 互异版（525 判定版互补，0-1 背包 DP） |
| 527_oddpart | 526_distinct | 奇部版（526 互异版互补，534 做对峙） |
| 528_pent | 350_binom | 递推版（350 乘法式互补，五边形数定理） |
| 529_coinchange | 527_oddpart | 面值版（527 奇数列互补，{1,2,5} 三段直写） |
| 530_compose | 526_distinct | 有序版（526 无序版互补，2 幂） |
| 531_stirling2 | 351_catalan | 加权版（351 卷积版互补，拍扁二维表） |
| 532_bell | 531_stirling2 | 行和版（531 单格版互补；453 集合版互补） |
| 533_noones | 529_coinchange | 去 1 版（529 面值版互补，{2..n} 全集） |
| 534_euler_duel | 526_distinct | 对峙版（526×527 双实现锁定） |
| 535_coin_duel | 529_coinchange | 对峙版（529 DP 版互补，加暴力枚举） |
| 536_p8 | 528_pent | 背包版（528 递推版互补，543 做对峙） |
| 537_rr1 | 534_euler_duel | RR 版（534 Euler 版互补，差分递归对峙） |
| 538_selfconj | 526_distinct | 奇限版（526 全硬币互补，奇硬币 0-1 背包） |
| 539_qgrow | 526_distinct | 高位版（526 低位版互补，541 做对峙） |
| 540_oddgrow | 527_oddpart | 高位版（527 低位版互补，541 做对峙） |
| 541_euler2 | 534_euler_duel | 高位版（534 低位版互补，539/540 双源锁定） |
| 542_pent_extend | 528_pent | 高位版（528 低位版互补，543 做对峙） |
| 543_duel_p | 536_p8 | 对峙版（536×542 双实现锁定） |
| 544_largest | 531_stirling2 | 精确版（531 加权版互补，545 做行和） |
| 545_tri_p | 536_p8 | 三向版（536×542×544 三算法互锁） |
| 546_pascal | 531_stirling2 | 建表版（531 首零版互补，此处首列恒一） |
| 547_rowsum | 546_pascal | 行和版（546 单格版互补，550 做对峙） |
| 548_hockey | 547_rowsum | 对角版（547 横向版互补，551 做对峙） |
| 549_vander | 548_hockey | 卷积版（548 单表版互补，552 做对峙） |
| 550_duel_row | 547_rowsum | 对峙版（547 表求和与 2^n 闭式锁定） |
| 551_duel_hockey | 548_hockey | 对峙版（548 求和与查表锁定） |
| 552_duel_vander | 549_vander | 对峙版（549 卷积与查表锁定） |
| 553_sym | 546_pascal | 对称版（546 单查版互补，双下标互锁） |
| 554_pascal_row | 350_binom | 整行版（350 单值版互补，乘法式逐项） |
| 555_tri_binom | 546_pascal | 三向版（546×350×549 三算法互锁） |
| 556_cat_conv | 351_catalan | 卷积版（351 商式版互补，561 做对峙） |
| 557_central | 350_binom | 取顶版（350 单值版延展，`2n` 取顶） |
| 558_ballot | 351_catalan | 双参版（351 单参版互补，`q=p` 退化即 Catalan） |
| 559_dyck | 531_stirling2 | 网格版（531 三角版互补，对角守卫） |
| 560_fuss | 351_catalan | 三元版（351 二元版互补，564 做对峙） |
| 561_duel_cat | 556_cat_conv | 对峙版（556×351 双实现锁定） |
| 562_duel_dyck | 559_dyck | 对峙版（559×556 双视角锁定） |
| 563_duel_central | 546_pascal | 对峙版（546×557 双实现锁定） |
| 564_duel_fuss | 560_fuss | 对峙版（560 商式与三元卷积锁定） |
| 565_tri_cat | 556_cat_conv | 三向版（556×351×559 三算法互锁） |
| 566_derange | 125_fact | 递推版（125 阶乘版互补，568 做对峙） |
| 567_derange_sum | 566_derange | 容斥版（566 递推版互补，568 做对峙） |
| 568_duel_derange | 566_derange | 对峙版（566×567 双实现锁定） |
| 569_euler | 531_stirling2 | 双权版（531 单权版互补，570 做行和） |
| 570_euler_row | 569_euler | 行和版（569 单格版互补，571 做对峙） |
| 571_duel_euler | 570_euler_row | 对峙版（570 行和与阶乘闭式锁定） |
| 572_perm | 350_binom | 连乘版（350 除法版互补，573 做对峙） |
| 573_duel_permfact | 572_perm | 对峙版（572 降乘与升乘阶乘锁定） |
| 574_euler_sym | 569_euler | 对称版（569 单查版互补，镜像下标互锁） |
| 575_tri_derange | 566_derange | 三向版（566×567 双源加单项式三锁） |
| 576_stirling1 | 531_stirling2 | 一类版（531 二类版互补，580 做行值） |
| 577_rising | 572_perm | 上升版（572 下降版互补，579 做对峙） |
| 578_falling | 577_rising | 下降版（577 上升版互补，579 做对峙） |
| 579_duel_risefall | 577_rising | 对峙版（577×578 升降双实现锁定） |
| 580_rowval | 576_stirling1 | 行值版（576 单格版互补，581 做对峙） |
| 581_duel_rowval | 580_rowval | 对峙版（580 行和与阶乘闭式锁定） |
| 582_col | 576_stirling1 | 首列版（576 全表版互补，583 做对峙） |
| 583_duel_col | 582_col | 对峙版（582 首列与错位阶乘锁定） |
| 584_duel_diag | 576_stirling1 | 次对角版（576 全表版互补，对换解释） |
| 585_tri_s1 | 580_rowval | 三向版（580×125×577 三算法互锁） |
| 586_lah | 558_ballot | 商式版（558 同族互补，590 做对峙） |
| 587_lah_row | 586_lah | 行版（586 单格版互补，行和直打） |
| 588_fubini | 532_bell | 加权版（532 无权版互补，591 做对峙） |
| 589_lah_rec | 586_lah | 递推版（586 商式版互补，590 做对峙） |
| 590_duel_lah | 586_lah | 对峙版（586×589 双实现锁定） |
| 591_duel_fubini | 588_fubini | 对峙版（588 行和与组合递推锁定） |
| 592_lah_col | 586_lah | 首列版（586 全表版互补，594 做对峙） |
| 593_fubini6 | 588_fubini | 高位版（588 低位版互补，大值单锁） |
| 594_duel_lahcol | 592_lah_col | 对峙版（592 首列与阶乘直对锁定） |
| 595_tri_lah | 586_lah | 三向版（586×589 双源加商式B三锁） |
| 596_narayana | 586_lah | 细化版（586 同族互补，599 做行和对峙） |
| 597_narayana_row | 596_narayana | 行版（596 单格版互补，行和直打） |
| 598_narayana6 | 597_narayana_row | 高位版（597 低位版互补，600 做对峙） |
| 599_duel_narrow | 597_narayana_row | 对峙版（597 行和与 556 卷积锁定） |
| 600_duel_narcat | 598_narayana6 | 对峙版（598 行和与 351 商式锁定） |
| 601_narayana_sym | 596_narayana | 对称版（596 单查版互补，镜像互锁） |
| 602_narayana_edge | 596_narayana | 边值版（596 全表版互补，603 做对峙） |
| 603_duel_edge | 602_narayana_edge | 对峙版（602 边值与三角数锁定） |
| 604_narayana_alt | 596_narayana | 移位版（596 商式A互补，605 做三向） |
| 605_tri_narcat | 598_narayana6 | 三向版（598×556×351 三算法互锁） |
| 606_schroder | 351_catalan | 姐妹版（Catalan 同族格路计数，609 做对峙） |
| 607_schro_seq | 606_schroder | 序列版（606 单值版互补，前5项和直打） |
| 608_schro_high | 606_schroder | 高位版（606 低位版互补，610 做对峙） |
| 609_duel_schro | 606_schroder | 对峙版（606 迭代与递归双实现锁定） |
| 610_duel_schro_high | 608_schro_high | 对峙版（608 高位与递归双实现锁定） |
| 611_delannoy_tab | 464_duel_delannoy | 表版（464 格路版互补，546 范式拍扁，612 做对峙） |
| 612_duel_delan | 611_delannoy_tab | 对峙版（611 建表与二项直和锁定） |
| 613_schro6 | 608_schro_high | 高位版（608 高位版互补，614 做对峙） |
| 614_duel_schro6 | 613_schro6 | 对峙版（613 高位与递归双实现锁定） |
| 615_tri_schro | 608_schro_high | 三向版（迭代×递归×商式三算法互锁） |
| 616_delannoy_row | 611_delannoy_tab | 行版（611 建表版互补，高位行和直打） |
| 617_delannoy_high | 611_delannoy_tab | 高位版（611 低位版互补，618 做对峙） |
| 618_duel_delan_high | 617_delannoy_high | 对峙版（617 高位与直和双实现锁定） |
| 619_delannoy_sym | 611_delannoy_tab | 对称版（611 建表版互补，行列镜像互锁） |
| 620_delannoy_central | 617_delannoy_high | 中央版（617 高位版互补，621 做对峙） |
| 621_duel_central | 620_delannoy_central | 对峙版（620 中央与直和双实现锁定） |
| 622_schro_delan | 606_schroder | 桥接版（606×611 两族差式单锁，623 做对峙） |
| 623_duel_bridge | 622_schro_delan | 对峙版（622 桥式与递推跨族锁定） |
| 624_delannoy6 | 617_delannoy_high | 高位版（617 高位版互补，625 做三向） |
| 625_tri_delan | 617_delannoy_high | 三向版（建表×直和×容斥三算法互锁） |
| 626_motzkin | 606_schroder | 姐妹版（Schröder 同族格路计数，629 做对峙） |
| 627_motz_seq | 626_motzkin | 序列版（626 单值版互补，前6项和直打） |
| 628_motz_high | 626_motzkin | 高位版（626 低位版互补，630 做对峙） |
| 629_duel_motz | 626_motzkin | 对峙版（626 迭代与递归双实现锁定） |
| 630_duel_motz_high | 628_motz_high | 对峙版（628 高位与递归双实现锁定） |
| 631_motz7 | 628_motz_high | 高位版（628 高位版互补，632 做对峙） |
| 632_duel_motz7 | 631_motz7 | 对峙版（631 高位与直和双实现锁定） |
| 633_motz_closed | 626_motzkin | 直和版（626 递推版互补，635 做三向） |
| 634_motz8 | 631_motz7 | 高位版（631 高位版互补，635 做三向） |
| 635_tri_motz | 628_motz_high | 三向版（迭代×递归×直和三算法互锁） |
| 636_riordan | 626_motzkin | 姐妹版（Motzkin 姐妹序列，639 做对峙） |
| 637_rio_seq | 636_riordan | 序列版（636 单值版互补，前7项和直打） |
| 638_rio_high | 636_riordan | 高位版（636 低位版互补，640 做对峙） |
| 639_duel_rio | 636_riordan | 对峙版（636 短递推与卷积双实现锁定） |
| 640_duel_rio_high | 638_rio_high | 对峙版（638 高位与卷积双实现锁定） |
| 641_rio7 | 638_rio_high | 高位版（638 高位版互补，642 做对峙） |
| 642_duel_rio7 | 641_rio7 | 对峙版（641 高位与卷积双实现锁定） |
| 643_rio_bridge | 636_riordan | 桥接版（636×626 两族差序单锁，645 做三向） |
| 644_rio8 | 641_rio7 | 高位版（641 高位版互补，645 做三向） |
| 645_tri_rio | 638_rio_high | 三向版（短递推×卷积×桥序三算法互锁） |
| 646_ballot | 351_catalan | 三角版（Catalan 对角线全三角，649 做对峙） |
| 647_ballot_row | 646_ballot | 行版（646 单格版互补，行和恒等直打） |
| 648_ballot_high | 646_ballot | 高位版（646 低位版互补，650 做对峙） |
| 649_duel_ballot | 646_ballot | 对峙版（646 建表与商式双实现锁定） |
| 650_duel_ballot_high | 648_ballot_high | 对峙版（648 高位与商式双实现锁定） |
| 651_ballot_reflect | 646_ballot | 差式版（646 建表版互补，655 做三向） |
| 652_duel_reflect | 646_ballot | 对峙版（646 建表与差式跨算法锁定） |
| 653_ballot5 | 647_ballot_row | 行版（647 低位行互补，高位行和再锁） |
| 654_ballot_diag | 351_catalan | 对角版（351 单值版互补，655 做三向） |
| 655_tri_ballot | 648_ballot_high | 三向版（建表×商式×差式三算法互锁） |
| 656_fine | 351_catalan | 姐妹版（Catalan 姐妹序列，659 做对峙） |
| 657_fine_seq | 656_fine | 序列版（656 单值版互补，前7项和直打） |
| 658_fine_high | 656_fine | 高位版（656 低位版互补，660 做对峙） |
| 659_duel_fine | 656_fine | 对峙版（656 短递推与桥序双实现锁定） |
| 660_duel_fine_high | 658_fine_high | 对峙版（658 高位与桥序双实现锁定） |
| 661_fine7 | 658_fine_high | 高位版（658 高位版互补，662 做对峙） |
| 662_duel_fine7 | 661_fine7 | 对峙版（661 高位与卷积双实现锁定） |
| 663_fine_bridge | 656_fine | 桥接版（656×351 两族差序单锁，665 做三向） |
| 664_fine8 | 661_fine7 | 高位版（661 高位版互补，665 做三向） |
| 665_tri_fine | 658_fine_high | 三向版（短递推×卷积×桥序三算法互锁） |
| 666_eulerian | 646_ballot | 三角版（Ballot 同族加权三角，669 做对峙） |
| 667_eul_row | 666_eulerian | 行版（666 单格版互补，行和恒等直打） |
| 668_eul_high | 666_eulerian | 高位版（666 低位版互补，670 做对峙） |
| 669_duel_eul | 666_eulerian | 对峙版（666 建表与交错和双实现锁定） |
| 670_duel_eul_high | 668_eul_high | 对峙版（668 高位与交错和双实现锁定） |
| 671_eul_sym | 666_eulerian | 对称版（666 单格版互补，镜像双算一致） |
| 672_eul_alt | 666_eulerian | 交错版（666 建表版互补，675 做三向） |
| 673_eul_row6 | 667_eul_row | 行版（667 低位行互补，高位行和再锁） |
| 674_eul_fact | 667_eul_row | 桥接版（667×230 两族行和恒等单锁，675 做三向） |
| 675_tri_eul | 668_eul_high | 三向版（建表行和×交错行和×阶乘三算法互锁） |
| 676_eulb | 666_eulerian | B型版（A 型姐妹符号置换，679 做对峙） |
| 677_eulb_row | 676_eulb | 行版（676 单格版互补，行和恒等直打） |
| 678_eulb_high | 676_eulb | 高位版（676 低位版互补，680 做对峙） |
| 679_duel_eulb | 676_eulb | 对峙版（676 建表与交错和双实现锁定） |
| 680_duel_eulb_high | 678_eulb_high | 对峙版（678 高位与交错和双实现锁定） |
| 681_eulb_sym | 676_eulb | 对称版（676 单格版互补，镜像双算一致） |
| 682_eulb_alt | 676_eulb | 交错版（676 建表版互补，685 做三向） |
| 683_eulb_row5 | 677_eulb_row | 行版（677 低位行互补，高位行和再锁） |
| 684_eulb_fact | 677_eulb_row | 桥接版（677×230 两族行和恒等单锁，685 做三向） |
| 685_tri_eulb | 678_eulb_high | 三向版（建表行和×交错行和×公式三算法互锁） |
| 686_stir17 | 576_stirling1 | 高位版（一类低位版互补，689 做对峙） |
| 687_stir1_row7 | 686_stir17 | 行版（686 单格版互补，行和恒等直打） |
| 688_stir18 | 686_stir17 | 高位版（686 n=7 版互补，690 做对峙） |
| 689_duel_stir17 | 686_stir17 | 对峙版（686 建表与递归双实现锁定） |
| 690_duel_stir18 | 688_stir18 | 对峙版（688 高位与递归双实现锁定） |
| 691_stir1_col1 | 686_stir17 | 首列版（686 内格版互补，692 做首列对峙） |
| 692_duel_col1 | 691_stir1_col1 | 对峙版（691 首列与阶乘跨族锁定） |
| 693_stir1_edge | 686_stir17 | 边值版（686 内格版互补，695 做三向） |
| 694_stir1_row8 | 687_stir1_row7 | 行版（687 低位行互补，高位行和再锁） |
| 695_tri_stir1 | 687_stir1_row7 | 三向版（建表行和×递归行和×阶乘三算法互锁） |
| 696_stir27 | 531_stirling2 | 高位版（二类低位版互补，699 做对峙） |
| 697_stir2_row7 | 696_stir27 | 行版（696 单格版互补，行和恒等直打） |
| 698_stir28 | 696_stir27 | 高位版（696 n=7 版互补，700 做对峙） |
| 699_duel_stir27 | 696_stir27 | 对峙版（696 建表与递归双实现锁定） |
| 700_duel_stir28 | 698_stir28 | 对峙版（698 高位与递归双实现锁定） |
| 701_stir2_col2 | 696_stir27 | 列版（696 内格版互补，702 做列对峙） |
| 702_duel_col2 | 701_stir2_col2 | 对峙版（701 列与幂公式跨算法锁定） |
| 703_stir2_bell | 697_stir2_row7 | 桥接版（697×532 两族行和恒等单锁，705 做三向） |
| 704_stir29 | 698_stir28 | 高位版（698 n=8 版互补，705 做三向） |
| 705_tri_stir2 | 697_stir2_row7 | 三向版（建表行和×递归行和×Bell数三算法互锁） |
| 706_lah7 | 586_lah | 高位版（Lah 低位版互补，709 做对峙） |
| 707_lah_row7 | 706_lah7 | 行版（706 单格版互补，行和恒等直打） |
| 708_lah8 | 706_lah7 | 高位版（706 n=7 版互补，710 做对峙） |
| 709_duel_lah7 | 706_lah7 | 对峙版（706 建表与商式双实现锁定） |
| 710_duel_lah8 | 708_lah8 | 对峙版（708 高位与商式双实现锁定） |
| 711_lah_col1 | 706_lah7 | 首列版（706 内格版互补，712 做首列对峙） |
| 712_duel_lahcol | 711_lah_col1 | 对峙版（711 首列与阶乘跨族锁定） |
| 713_lah_edge | 706_lah7 | 边值版（706 内格版互补，715 做三向） |
| 714_lah_row8 | 707_lah_row7 | 行版（707 低位行互补，高位行和再锁） |
| 715_tri_lah | 707_lah_row7 | 三向版（建表行和×商式行和×递归行和三算法互锁） |
| 716_nar7 | 596_narayana | 高位版（Narayana 低位版互补，719 做对峙） |
| 717_nar_row7 | 716_nar7 | 行版（716 单格版互补，行和恒等直打） |
| 718_nar8 | 716_nar7 | 高位版（716 n=7 版互补，720 做对峙） |
| 719_duel_nar7 | 717_nar_row7 | 对峙版（717 行和与商式双实现锁定） |
| 720_duel_nar8 | 718_nar8 | 对峙版（718 高位与商式双实现锁定） |
| 721_nar_sym | 716_nar7 | 对称版（716 单格版互补，镜像双算一致） |
| 722_nar_edge | 716_nar7 | 边值版（716 内格版互补，725 做三向） |
| 723_nar_col2 | 716_nar7 | 列版（716 单格版互补，纵深三值锁定） |
| 724_nar_row8 | 717_nar_row7 | 行版（717 低位行互补，高位行和再锁） |
| 725_tri_nar2 | 717_nar_row7 | 三向版（行和×商式×卷积三算法互锁） |
| 726_cat_conv6 | 556_cat_conv | 卷积版（556 低位版互补，728 做对峙） |
| 727_cat_div8 | 351_catalan | 商式版（351 低位版互补，728 做对峙） |
| 728_duel_cat | 726_cat_conv6 | 对峙版（726 卷积与商式双实现锁定） |
| 729_central_binom | 557_central | 中央版（557 取顶版互补，730 做对峙） |
| 730_duel_central | 729_central_binom | 对峙版（729 商式与建表双实现锁定） |
| 731_cat_diff | 351_catalan | 差式版（351 商式版互补，735 做三向） |
| 732_duel_catdiff | 731_cat_diff | 对峙版（731 差式与商式跨算法锁定） |
| 733_cat_ratio | 351_catalan | 比值版（351 商式版互补，735 做三向） |
| 734_cat_conv8 | 726_cat_conv6 | 卷积版（726 低位版互补，735 做三向） |
| 735_tri_cat | 727_cat_div8 | 三向版（商式×卷积×比值三算法互锁） |
| 736_fub7 | 588_fubini | 高位版（Fubini 低位版互补，739 做对峙） |
| 737_fub_seq | 736_fub7 | 序列版（736 单值版互补，前8项和直打） |
| 738_fub8 | 736_fub7 | 高位版（736 n=7 版互补，740 做对峙） |
| 739_duel_fub7 | 736_fub7 | 对峙版（736 递推与加权双实现锁定） |
| 740_duel_fub8 | 738_fub8 | 对峙版（738 高位与加权双实现锁定） |
| 741_fubw | 588_fubini | 加权版（588 低位版互补，745 做三向） |
| 742_fubp | 736_fub7 | 表版（736 商版互补，743 做表商对峙） |
| 743_duel_fubp | 742_fubp | 对峙版（742 表版与商版同核异源锁定） |
| 744_fubp8 | 742_fubp | 高位版（742 n=7 版互补，745 做三向） |
| 745_tri_fub | 736_fub7 | 三向版（商版递推×加权和×表版递推三算法互锁） |
| 746_motz9 | 634_motz8 | 高位版（Motzkin n=8 版互补，749 做对峙） |
| 747_motz_seq9 | 746_motz9 | 序列版（746 单值版互补，前10项和直打） |
| 748_motz10 | 746_motz9 | 高位版（746 n=9 版互补，750 做对峙） |
| 749_duel_motz9 | 746_motz9 | 对峙版（746 迭代与直和双实现锁定） |
| 750_duel_motz10 | 748_motz10 | 对峙版（748 高位与直和双实现锁定） |
| 751_motz_riob9 | 643_rio_bridge | 桥接版（643 低位桥互补，753 做续桥对峙） |
| 752_rio910 | 644_rio8 | 高位版（644 n=8 版互补，753 做续桥对峙） |
| 753_duel_rio910 | 751_motz_riob9 | 对峙版（751 续桥与短递推双实现锁定） |
| 754_motz_alt910 | 633_motz_closed | 直和版（633 低位版互补，755 做三向） |
| 755_tri_motz9 | 746_motz9 | 三向版（迭代×直和×桥序和三算法互锁） |
| 756_delan7 | 611_delannoy_tab | 高位版（Delannoy n≤6 版互补，759 做对峙） |
| 757_delan_row7 | 756_delan7 | 行版（756 单格版互补，行和恒等直打） |
| 758_delan_high7 | 624_delannoy6 | 中央版（624 D66 版互补，759 做对峙） |
| 759_duel_delan7 | 758_delan_high7 | 对峙版（758 中央与直和双实现锁定） |
| 760_duel_delan76 | 758_delan_high7 | 对峙版（758 高位与直和双实现锁定） |
| 761_delan_sym7 | 756_delan7 | 对称版（756 单格版互补，镜像双算一致） |
| 762_delan_central7 | 620_delannoy_central | 中央版（620 低位版互补，765 做三向） |
| 763_delan_col7 | 756_delan7 | 列版（756 行视角互补，纵深三值锁定） |
| 764_duel_delan_col7 | 763_delan_col7 | 对峙版（763 列与直和双实现锁定） |
| 765_tri_delan7 | 758_delan_high7 | 三向版（建表×直和×容斥三算法互锁） |
| 766_sschro | 606_schroder | 小版（606 大数姐妹，769 做对峙） |
| 767_sschro_seq | 766_sschro | 序列版（766 单值版互补，前7项和直打） |
| 768_sschro_high | 766_sschro | 高位版（766 低位版互补，770 做对峙） |
| 769_duel_sschro | 766_sschro | 对峙版（766 短递推与半大数双实现锁定） |
| 770_duel_sschro_high | 768_sschro_high | 对峙版（768 高位与半大数双实现锁定） |
| 771_sschro8 | 768_sschro_high | 高位版（768 n=7 版互补，772 做对峙） |
| 772_duel_sschro8 | 771_sschro8 | 对峙版（771 高位与半大数双实现锁定） |
| 773_schro78 | 613_schro6 | 高位版（613 S6 版互补，775 做三向） |
| 774_schro_bridge | 769_duel_sschro | 桥接版（769 对峙版互补，桥大端三锁） |
| 775_tri_sschro | 768_sschro_high | 三向版（短递推×半大数×半递归三算法互锁） |
| 776_bell9 | 703_stir2_bell | 高位版（Bell n≤8 版互补，779 做对峙） |
| 777_bell_seq | 776_bell9 | 序列版（776 单值版互补，前10项和直打） |
| 778_bell10 | 776_bell9 | 高位版（776 n=9 版互补，780 做对峙） |
| 779_duel_bell9 | 776_bell9 | 对峙版（776 递推与三角双实现锁定） |
| 780_duel_bell10 | 778_bell10 | 对峙版（778 高位与三角双实现锁定） |
| 781_belltri | 776_bell9 | 三角版（776 递推版互补，785 做三向） |
| 782_bell_rowsum | 697_stir2_row7 | 桥接版（697×776 两族行和恒等单锁，785 做三向） |
| 783_duel_bellrow | 782_bell_rowsum | 对峙版（782 行和与递推跨族锁定） |
| 784_bell_parity | 776_bell9 | 奇偶版（776 单值版互补，周期三值锁定） |
| 785_tri_bell | 776_bell9 | 三向版（二项递推×三角×行和三算法互锁） |
| 786_schro9 | 773_schro78 | 高位版（大续 S78 版互补，789 做对峙） |
| 787_schro_seq9 | 786_schro9 | 序列版（786 单值版互补，前10项和直打） |
| 788_schro10 | 786_schro9 | 高位版（786 n=9 版互补，790 做对峙） |
| 789_duel_schro9 | 786_schro9 | 对峙版（786 迭代与商式双实现锁定） |
| 790_duel_schro10 | 788_schro10 | 对峙版（788 高位与商式双实现锁定） |
| 791_sschro910 | 771_sschro8 | 高位版（771 n=8 版互补，792 做续桥对峙） |
| 792_duel_sschro910 | 791_sschro910 | 对峙版（791 续桥与半大数双实现锁定） |
| 793_schro_qseq | 789_duel_schro9 | 商式版（789 对峙版互补，795 做三向） |
| 794_schro_bridge9 | 774_schro_bridge | 桥接版（774 低位桥互补，桥续大端双锁） |
| 795_tri_schro9 | 786_schro9 | 三向版（迭代×商式×倍小三算法互锁） |
| 796_eul7 | 666_eulerian | 高位版（Eulerian n≤6 版互补，799 做对峙） |
| 797_eul_row7 | 796_eul7 | 行版（796 单格版互补，行和恒等直打） |
| 798_eul8 | 796_eul7 | 高位版（796 n=7 版互补，800 做对峙） |
| 799_duel_eul7 | 796_eul7 | 对峙版（796 建表与交错和双实现锁定） |
| 800_duel_eul8 | 798_eul8 | 对峙版（798 高位与交错和双实现锁定） |
| 801_eul_sym78 | 796_eul7 | 对称版（796 单格版互补，镜像双算一致） |
| 802_eul_alt78 | 672_eul_alt | 交错版（672 低位版互补，805 做三向） |
| 803_eul_row8 | 797_eul_row7 | 行版（797 低位行互补，高位行和再锁） |
| 804_eul_fact78 | 797_eul_row7 | 桥接版（797×230 两族行和恒等单锁，805 做三向） |
| 805_tri_eul78 | 798_eul8 | 三向版（建表行和×交错行和×阶乘三算法互锁） |
| 806_eulb6 | 676_eulb | 高位版（B 型 n≤5 版互补，809 做对峙） |
| 807_eulb_row6 | 806_eulb6 | 行版（806 单格版互补，行和恒等直打） |
| 808_eulb7 | 806_eulb6 | 高位版（806 n=6 版互补，810 做对峙） |
| 809_duel_eulb6 | 806_eulb6 | 对峙版（806 建表与交错和双实现锁定） |
| 810_duel_eulb7 | 808_eulb7 | 对峙版（808 高位与交错和双实现锁定） |
| 811_eulb_sym67 | 806_eulb6 | 对称版（806 单格版互补，镜像双算一致） |
| 812_eulb_alt67 | 682_eulb_alt | 交错版（682 低位版互补，815 做三向） |
| 813_eulb_row7 | 807_eulb_row6 | 行版（807 低位行互补，高位行和再锁） |
| 814_eulb_fact67 | 807_eulb_row6 | 桥接版（807×684 两族行和恒等单锁，815 做三向） |
| 815_tri_eulb67 | 808_eulb7 | 三向版（建表行和×交错行和×公式三算法互锁） |
| 816_stir19 | 686_stir17 | 高位版（一类 n=7 版互补，819 做对峙） |
| 817_stir1_row9 | 816_stir19 | 行版（816 单格版互补，行和恒等直打） |
| 818_duel_col19 | 691_stir1_col1 | 对峙版（691 首列与阶乘跨族续高位锁定） |
| 819_duel_stir19 | 816_stir19 | 对峙版（816 建表与递归双实现锁定） |
| 820_duel_stir194 | 816_stir19 | 对峙版（816 内格与递归双实现锁定） |
| 821_stir1_edge9 | 693_stir1_edge | 边值版（693 低位版互补，825 做三向） |
| 822_stir1_col29 | 816_stir19 | 列版（816 单格版互补，纵深三值锁定） |
| 823_duel_col29 | 822_stir1_col29 | 对峙版（822 列与递归双实现锁定） |
| 824_stir19_right | 816_stir19 | 右部版（816 左部版互补，全行覆盖） |
| 825_tri_stir19 | 817_stir1_row9 | 三向版（建表行和×递归行和×阶乘三算法互锁） |
| 826_stir29 | 696_stir27 | 高位版（二类 n=7 版互补，829 做对峙） |
| 827_stir2_row10 | 826_stir29 | 行版（826 单格版互补，行和恒等直打） |
| 828_duel_col29 | 701_stir2_col2 | 对峙版（701 首列与幂式跨族续高位锁定） |
| 829_duel_stir29 | 826_stir29 | 对峙版（826 建表与递归双实现锁定） |
| 830_duel_stir2910 | 826_stir29 | 对峙版（826 内格与递归双实现锁定） |
| 831_stir2_edge10 | 821_stir1_edge9 | 边值版（一类边值跨族互补，835 做三向） |
| 832_stir2_col210 | 826_stir29 | 列版（826 单格版互补，纵深三值锁定） |
| 833_duel_col210 | 832_stir2_col210 | 对峙版（832 列与三角数双实现锁定） |
| 834_stir29_right | 826_stir29 | 右部版（826 左部版互补，全行覆盖） |
| 835_tri_stir29 | 827_stir2_row10 | 三向版（建表行和×Bell递推×Aitken三算法互锁） |
| 836_lah9 | 708_lah8 | 高位版（Lah n=8 版互补，839 做对峙） |
| 837_lah_row9 | 836_lah9 | 行版（836 单格版互补，行和恒等直打） |
| 838_duel_lahcol19 | 711_lah_col1 | 对峙版（711 首列与阶乘跨族续高位锁定） |
| 839_duel_lah9 | 836_lah9 | 对峙版（836 建表与商式双实现锁定） |
| 840_duel_lah910 | 836_lah9 | 对峙版（836 内格与商式双实现锁定） |
| 841_lah_edge9 | 713_lah_edge | 边值版（713 低位版互补，845 做三向） |
| 842_lah_col29 | 836_lah9 | 列版（836 单格版互补，纵深三值锁定） |
| 843_duel_lahcol29 | 842_lah_col29 | 对峙版（842 列与边式双实现锁定） |
| 844_lah9_right | 836_lah9 | 右部版（836 左部版互补，全行覆盖） |
| 845_tri_lah9 | 837_lah_row9 | 三向版（建表行和×商式行和×递归行和三算法互锁） |
| 846_nar9 | 718_nar8 | 高位版（Narayana n=8 版互补，849 做对峙） |
| 847_nar_row9 | 846_nar9 | 行版（846 单格版互补，行和恒等直打） |
| 848_duel_narcol19 | 722_nar_edge | 对峙版（722 边值与三角数跨族续高位锁定） |
| 849_duel_nar9 | 846_nar9 | 对峙版（846 乘式与表式双实现锁定） |
| 850_duel_nar910 | 847_nar_row9 | 对峙版（847 行源与商式双实现锁定） |
| 851_nar_edge9 | 722_nar_edge | 边值版（722 低位版互补，855 做三向） |
| 852_nar_col29 | 846_nar9 | 列版（846 单格版互补，纵深三值锁定） |
| 853_duel_narcol29 | 852_nar_col29 | 对峙版（852 列与表式双实现锁定） |
| 854_nar9_right | 846_nar9 | 右部版（846 左部版互补，全行覆盖） |
| 855_tri_nar9 | 847_nar_row9 | 三向版（行和×Catalan商式×Catalan卷积三算法互锁） |
| 856_cat9 | 727_cat_div8 | 高位版（商式 n=7/8 版互补，858 做对峙） |
| 857_cat_conv9 | 726_cat_conv6 | 卷积版（726 低位版互补，858 做对峙） |
| 858_duel_cat9 | 856_cat9 | 对峙版（856 商源与卷积双实现锁定） |
| 859_central18 | 729_central_binom | 分子版（729 低位版互补，860 做对峙） |
| 860_duel_central9 | 859_central18 | 对峙版（859 乘式与建表双实现锁定） |
| 861_cat_diff9 | 731_cat_diff | 差式版（731 低位版互补，865 做三向） |
| 862_duel_catdiff9 | 861_cat_diff9 | 对峙版（861 差源与商源合流） |
| 863_cat_ratio9 | 733_cat_ratio | 比值版（733 低位版互补，865 做三向） |
| 864_cat_conv10 | 857_cat_conv9 | 卷积版（857 n=9 版互补，865 做三向） |
| 865_tri_cat9 | 856_cat9 | 三向版（商式×卷积×比值递推三算法互锁） |
| 866_fub9 | 738_fub8 | 高位版（Fubini n=8 版互补，868 做对峙） |
| 867_fubw9 | 866_fub9 | 加权版（866 递推版互补，873 做对峙） |
| 868_duel_fub9 | 866_fub9 | 对峙版（866 递推与加权双实现锁定） |
| 869_fubseq9 | 866_fub9 | 序列版（866 单值版互补，前缀和直打） |
| 870_duel_fubp9 | 866_fub9 | 对峙版（866 表版与商版双实现锁定） |
| 871_fubw9d | 867_fubw9 | 分项版（867 总量版互补，加权分项直打） |
| 872_fubp9 | 744_fubp8 | 表版（744 n=8 版互补，875 做三向） |
| 873_duel_fubw9 | 872_fubp9 | 对峙版（872 表源与加权源合流） |
| 874_fub10 | 866_fub9 | 高位版（866 n=9 版互补，875 做三向） |
| 875_tri_fub9 | 866_fub9 | 三向版（商版递推×加权和×表版递推三算法互锁） |
| 876_motz11 | 748_motz10 | 高位版（Motzkin n=10 版互补，878 做对峙） |
| 877_motz_seq11 | 876_motz11 | 序列版（876 单值版互补，前缀和直打） |
| 878_duel_motz11 | 876_motz11 | 对峙版（876 迭代与直和双实现锁定） |
| 879_motz12 | 876_motz11 | 高位版（876 n=11 版互补，880 做对峙） |
| 880_duel_motz12 | 879_motz12 | 对峙版（879 迭代与直和双实现锁定） |
| 881_motz_riob11 | 876_motz11 | 桥接版（751 两族桥接续高位，883 做对峙） |
| 882_rio1112 | 752_rio910 | 高位版（752 n=9/10 版互补，883 做对峙） |
| 883_duel_rio1112 | 881_motz_riob11 | 对峙版（881 短递推与桥序双实现锁定） |
| 884_motz_alt1112 | 754_motz_alt910 | 直和版（754 低位版互补，885 做三向） |
| 885_tri_motz11 | 876_motz11 | 三向版（迭代×直和×桥序和三算法互锁） |
| 886_delan8 | 756_delan7 | 高位版（Delannoy n=7 版互补，889 做对峙） |
| 887_delan_row8 | 886_delan8 | 行版（886 单格版互补，行和恒等直打） |
| 888_delan_high8 | 758_delan_high7 | 中央版（758 D(7,7) 版互补，889 做对峙） |
| 889_duel_delan8 | 888_delan_high8 | 对峙版（888 建表与直和双实现锁定） |
| 890_duel_delan86 | 887_delan_row8 | 对峙版（887 内格与直和双实现锁定） |
| 891_delan_sym8 | 886_delan8 | 对称版（886 单格版互补，行列镜像互锁） |
| 892_delan_central8 | 762_delan_central7 | 中央版（762 低位版互补，895 做三向） |
| 893_delan_col8 | 886_delan8 | 列版（886 行视角互补，同值列复算） |
| 894_duel_delan_col8 | 893_delan_col8 | 对峙版（893 列与直和双实现锁定） |
| 895_tri_delan8 | 888_delan_high8 | 三向版（拍扁建表×二项直和×容斥和三算法互锁） |
| 896_schro11 | 788_schro10 | 高位版（大 Schröder n=10 版互补，898 做对峙） |
| 897_schro_seq11 | 896_schro11 | 序列版（896 单值版互补，前缀和直打） |
| 898_duel_schro11 | 896_schro11 | 对峙版（896 迭代与商式双实现锁定） |
| 899_schro12 | 896_schro11 | 高位版（896 n=11 版互补，900 做对峙） |
| 900_duel_schro12 | 899_schro12 | 对峙版（899 迭代与商式双实现锁定） |
| 901_sschro1112 | 791_sschro910 | 高位版（791 n=9/10 版互补，902 做对峙） |
| 902_duel_sschro1112 | 901_sschro1112 | 对峙版（901 短递推与半大数双实现锁定） |
| 903_schro_qseq11 | 793_schro_qseq | 纵深版（793 n=7/8/9 版互补，905 做三向） |
| 904_schro_bridge11 | 794_schro_bridge9 | 桥接版（794 n=9/10 桥互补，桥恒等续高位） |
| 905_tri_schro11 | 896_schro11 | 三向版（迭代×商式×倍小三算法互锁） |
| 906_bell11 | 778_bell10 | 高位版（Bell n=10 版互补，908 做对峙） |
| 907_bell_seq11 | 906_bell11 | 序列版（906 单值版互补，前缀和直打） |
| 908_duel_bell11 | 906_bell11 | 对峙版（906 递推与三角双实现锁定） |
| 909_bell12 | 906_bell11 | 高位版（906 n=11 版互补，910 做对峙） |
| 910_duel_bell12 | 909_bell12 | 对峙版（909 递推与三角双实现锁定） |
| 911_belltri11 | 781_belltri | 三角版（781 低位版互补，915 做三向） |
| 912_bell_rowsum11 | 906_bell11 | 行和版（906 递推版互补，915 做三向） |
| 913_duel_bellrow11 | 912_bell_rowsum11 | 对峙版（912 行源与递推源合流） |
| 914_bell_parity11 | 784_bell_parity | 奇偶版（784 n=8/9/10 版互补，周期续两位） |
| 915_tri_bell11 | 906_bell11 | 三向版（二项递推×Aitken三角×二类行和三算法互锁） |
| 916_eul9 | 798_eul8 | 高位版（Eulerian n=8 版互补，918 做对峙） |
| 917_eul_row9 | 916_eul9 | 行版（916 单格版互补，行和恒等直打） |
| 918_duel_eul9 | 916_eul9 | 对峙版（916 建表与交错双实现锁定） |
| 919_eul_sym9 | 916_eul9 | 对称版（916 单格版互补，行列镜像互锁） |
| 920_duel_eul910 | 917_eul_row9 | 对峙版（917 内格与交错双实现锁定） |
| 921_eul_alt9 | 802_eul_alt78 | 交错版（802 低位版互补，925 做三向） |
| 922_eul_fact9 | 917_eul_row9 | 桥接版（917 行源与阶乘互锁，925 做三向） |
| 923_duel_eulrow9 | 922_eul_fact9 | 对峙版（922 行源与交错源合流） |
| 924_eul9_right | 916_eul9 | 右部版（916 左部版互补，全行覆盖） |
| 925_tri_eul9 | 917_eul_row9 | 三向版（建表行和×交错和行和×阶乘三算法互锁） |

> 说明：TS 的高阶方法（`map/filter/reduce/find`）在 SLA 中用显式循环 + 闭包表达，
> 避免依赖尚未进入 direct-SAB 快路径的迭代器协议（roadmap Phase 6）。
> `for in` 协议、`async`、Node/Deno/NPM/Zod 相关 tsgosa demos（203+）不在本批范围。
