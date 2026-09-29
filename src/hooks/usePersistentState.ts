import { useEffect, useState } from "react";
export function usePersistentState<T>(
  key: string,
  initial: T,
  parse: (value: unknown) => T,
) {
  const [value, setValue] = useState<T>(() => {
    try {
      const saved = localStorage.getItem(key);
      return saved ? parse(JSON.parse(saved)) : initial;
    } catch {
      return initial;
    }
  });
  useEffect(() => {
    try {
      localStorage.setItem(key, JSON.stringify(value));
    } catch {
      /* Storage may be unavailable; retain the in-memory value. */
    }
  }, [key, value]);
  return [value, setValue] as const;
}
