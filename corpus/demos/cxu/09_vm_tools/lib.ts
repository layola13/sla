export function helper(x: number): number {
  return x * 2;
}

export function computeTotal(values: number[]): number {
  let total = 0;
  for (const v of values) {
    total += helper(v);
  }
  return total;
}
