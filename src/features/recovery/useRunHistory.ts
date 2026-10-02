import { useCallback, useEffect, useRef, useState } from "react";
import { queryRunHistory } from "./api";
import type { HistoryQuery, RunHistoryPage } from "./types";

export function useRunHistory(query: HistoryQuery = {}) {
  const [data, setData] = useState<RunHistoryPage | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const key = JSON.stringify(query);
  const generation = useRef(0);
  const refresh = useCallback(async () => {
    const current = ++generation.current;
    try {
      const page = await queryRunHistory(JSON.parse(key));
      if (current === generation.current) {
        setData(page);
        setError(page.storageError);
      }
    } catch (reason) {
      if (current === generation.current) setError(String(reason));
    } finally {
      if (current === generation.current) setLoading(false);
    }
  }, [key]);
  useEffect(() => {
    setLoading(true);
    void refresh();
    const timer = setInterval(() => void refresh(), 2500);
    return () => {
      generation.current++;
      clearInterval(timer);
    };
  }, [refresh]);
  return { data, error, loading, refresh };
}
