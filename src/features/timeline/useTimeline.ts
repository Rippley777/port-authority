import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";
import { queryTimeline } from "./api";
import type { TimelinePage, TimelineQuery } from "./types";
export const TimelineNavigation = createContext<(port?: number) => void>(
  () => {},
);
export function useOpenTimeline() {
  return useContext(TimelineNavigation);
}
function mergeHead(old: TimelinePage | null, page: TimelinePage): TimelinePage {
  if (!old || !page.events.length) return page;
  const ids = new Set(page.events.map((e) => e.id));
  const floor = page.events.at(-1)!.sequence;
  // A short page is exhaustive: don't retain records removed by retention or clearing.
  const older = page.nextCursor
    ? old.events.filter((e) => !ids.has(e.id) && e.sequence < floor)
    : [];
  return {
    ...page,
    events: [
      ...page.events.map((e) => old.events.find((p) => p.id === e.id) ?? e),
      ...older,
    ],
    nextCursor: older.length ? old.nextCursor : page.nextCursor,
  };
}
export function useTimeline(query: TimelineQuery) {
  const [data, setData] = useState<TimelinePage | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [pending, setPending] = useState<TimelinePage | null>(null);
  const key = JSON.stringify(query);
  const latest = useRef(key);
  latest.current = key;
  const generation = useRef(0);
  const busy = useRef<number | null>(null);
  const dataRef = useRef(data);
  dataRef.current = data;
  const refresh = useCallback(
    async (more = false) => {
      const version = generation.current;
      if (busy.current === version) return;
      busy.current = version;
      setLoading(true);
      try {
        const current = dataRef.current;
        const page = await queryTimeline({
          ...JSON.parse(key),
          ...(more && current?.nextCursor
            ? { before: current.nextCursor }
            : {}),
        });
        if (latest.current !== key || generation.current !== version) return;
        if (!page)
          throw new Error("History database did not return a response.");
        setError(page.error);
        if (more)
          setData((old) =>
            old
              ? {
                  ...page,
                  events: [
                    ...old.events,
                    ...page.events.filter(
                      (e) => !old.events.some((p) => p.id === e.id),
                    ),
                  ],
                }
              : page,
          );
        else if (
          current &&
          page.events.length &&
          page.events[0].id !== current.events[0]?.id &&
          (document.querySelector(".main-content")?.scrollTop ??
            window.scrollY) > 100
        )
          setPending(mergeHead(current, page));
        else {
          setData((old) => mergeHead(old, page));
          setPending(null);
        }
      } catch (e) {
        if (latest.current === key && generation.current === version)
          setError(String(e));
      } finally {
        if (busy.current === version) {
          busy.current = null;
          setLoading(false);
        }
      }
    },
    [key],
  );
  const reset = useCallback(() => {
    generation.current++;
    dataRef.current = null;
    setData(null);
    setPending(null);
    setError(null);
    void refresh();
  }, [refresh]);
  useEffect(() => {
    reset();
    const timer = setInterval(() => void refresh(), 2000);
    return () => {
      generation.current++;
      clearInterval(timer);
    };
  }, [refresh, reset]);
  return {
    data,
    error,
    loading,
    refresh,
    more: () => refresh(true),
    pending,
    showPending: () => {
      if (pending) {
        setData(pending);
        setPending(null);
      }
    },
    reset,
  };
}
