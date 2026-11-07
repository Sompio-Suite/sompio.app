import { useCallback, useEffect, useRef, useState } from "react";
import type { Page } from "./api";

export const PAGE_SIZE = 200;

/**
 * A list that can be millions of rows long but only ever loads the pages that
 * are on screen. Pair it with a virtualizer: call `ensure(index)` for visible rows.
 */
export function usePaged<T>(key: string | null, fetchPage: (offset: number, limit: number) => Promise<Page<T>>) {
  const [total, setTotal] = useState<number | null>(null);
  const [items, setItems] = useState<(T | undefined)[]>([]);
  const [error, setError] = useState<string | null>(null);
  const requested = useRef(new Set<number>());
  const generation = useRef(0);
  const fetchRef = useRef(fetchPage);
  fetchRef.current = fetchPage;

  const load = useCallback((page: number, gen: number) => {
    if (requested.current.has(page)) return;
    requested.current.add(page);
    fetchRef.current(page * PAGE_SIZE, PAGE_SIZE)
      .then((res) => {
        if (gen !== generation.current) return; // a newer query replaced this one
        setTotal(res.total);
        setItems((prev) => {
          const next = prev.length === res.total ? prev.slice() : Array.from({ length: res.total }, (_, i) => prev[i]);
          res.items.forEach((item, i) => (next[res.offset + i] = item));
          return next;
        });
      })
      .catch((e) => {
        requested.current.delete(page);
        if (gen === generation.current) setError(String(e));
      });
  }, []);

  useEffect(() => {
    generation.current += 1;
    requested.current = new Set();
    setItems([]);
    setTotal(null);
    setError(null);
    if (key !== null) load(0, generation.current);
  }, [key, load]);

  const ensure = useCallback((index: number) => load(Math.floor(index / PAGE_SIZE), generation.current), [load]);

  return { total, items, ensure, error };
}
