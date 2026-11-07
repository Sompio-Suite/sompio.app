import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { api, openLabel, openLink, platformName, type ChannelView, type MessageView } from "../api";
import type { Jump } from "../App";
import { channelKind, dateTime, fileName, n } from "../format";
import { usePaged } from "../usePaged";

interface Props {
  jump: Jump | null;
  onJumpHandled: () => void;
}

/** Waits until typing pauses, so searching stays smooth on huge histories. */
function useDebounced<T>(value: T, ms: number) {
  const [v, setV] = useState(value);
  useEffect(() => {
    const t = setTimeout(() => setV(value), ms);
    return () => clearTimeout(t);
  }, [value, ms]);
  return v;
}

export default function Messages({ jump, onJumpHandled }: Props) {
  const [filter, setFilter] = useState("");
  const [channels, setChannels] = useState<ChannelView[]>([]);
  const [current, setCurrent] = useState<string | null>(jump?.channelId ?? null);
  const [query, setQuery] = useState("");
  const [everywhere, setEverywhere] = useState(false);
  const [highlight, setHighlight] = useState<string | null>(null);
  const [revealed, setRevealed] = useState<Map<string, MessageView>>(new Map());
  const toggleReveal = async (m: MessageView) => {
    if (revealed.has(m.id)) {
      const next = new Map(revealed);
      next.delete(m.id);
      return setRevealed(next);
    }
    const full = await api.revealMessage(m.id);
    if (full) setRevealed(new Map(revealed).set(m.id, full));
  };
  const debouncedFilter = useDebounced(filter, 120);
  const debouncedQuery = useDebounced(query.trim(), 200);

  useEffect(() => {
    api.channels(debouncedFilter).then((list) => {
      setChannels(list);
      setCurrent((c) => c ?? list[0]?.id ?? null);
    });
  }, [debouncedFilter]);

  const searching = debouncedQuery.length > 0;
  const scope = everywhere ? null : current;
  const listKey = searching ? `s|${debouncedQuery}|${scope}` : current ? `c|${current}` : null;
  const { total, items, ensure } = usePaged<MessageView>(listKey, (o, l) =>
    searching ? api.search(debouncedQuery, scope, o, l) : api.messages(current!, o, l),
  );

  const parentRef = useRef<HTMLDivElement>(null);
  const v = useVirtualizer({
    count: total ?? 0,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 44,
    overscan: 12,
  });
  const virtualItems = v.getVirtualItems();
  useEffect(() => {
    virtualItems.forEach((r) => !items[r.index] && ensure(r.index));
  }, [virtualItems, items, ensure]);

  // Jump to a specific message (from the exposure report)
  useEffect(() => {
    if (!jump) return;
    setQuery("");
    setCurrent(jump.channelId);
    if (!jump.messageId) return onJumpHandled();
    api.messageIndex(jump.channelId, jump.messageId).then((idx) => {
      setHighlight(jump.messageId!);
      // let the channel load first, then scroll
      setTimeout(() => v.scrollToIndex(idx, { align: "center" }), 60);
      onJumpHandled();
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [jump]);

  // Start new conversations and searches at the right end: chats at the bottom (newest), searches at the top.
  useEffect(() => {
    if (total == null || jump?.messageId || highlight) return;
    if (!searching && total > 0) v.scrollToIndex(total - 1, { align: "end" });
    else parentRef.current?.scrollTo({ top: 0 });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [listKey, total == null]);

  const currentChannel = channels.find((c) => c.id === current);
  const words = useMemo(() => debouncedQuery.toLowerCase().split(/\s+/).filter(Boolean), [debouncedQuery]);

  return (
    <div className="messages">
      <aside className="chan-col" aria-label="Conversations">
        <div className="filter">
          <input type="search" placeholder="Find a conversation" value={filter} onChange={(e) => setFilter(e.target.value)} aria-label="Find a conversation" />
        </div>
        <div className="chan-list">
          {channels.map((c) => (
            <button
              key={c.id}
              className="chan"
              aria-current={c.id === current}
              onClick={() => {
                setCurrent(c.id);
                setHighlight(null);
                if (!everywhere) setQuery("");
              }}
            >
              <span style={{ minWidth: 0 }}>
                <span className="row-title" style={{ display: "block" }}>{c.name}</span>
                <span className="chan-kind">{platformName[c.platform]}, {channelKind[c.kind].toLowerCase()}</span>
              </span>
              <span className="small num">{n(c.message_count)}</span>
            </button>
          ))}
          {channels.length === 0 && <p className="muted small" style={{ padding: "0 0.6rem" }}>No conversation matches that name.</p>}
        </div>
      </aside>

      <section className="feed" aria-label="Messages">
        <div className="feed-head">
          <h3>{searching ? (everywhere ? "Search in all conversations" : `Search in ${currentChannel?.name ?? "this conversation"}`) : currentChannel?.name ?? "Messages"}</h3>
          <div className="toolbar">
            <input
              type="search"
              className="grow"
              style={{ flex: 1, width: "auto" }}
              placeholder="Search your messages"
              value={query}
              onChange={(e) => {
                setQuery(e.target.value);
                setHighlight(null);
              }}
              aria-label="Search your messages"
            />
            <label className="switch small">
              <input type="checkbox" checked={everywhere} onChange={(e) => setEverywhere(e.target.checked)} />
              All conversations
            </label>
          </div>
          {searching && total != null && <span className="small muted">{total === 1 ? "1 match" : `${n(total)} matches`}, newest first</span>}
        </div>

        <div className="feed-list" ref={parentRef}>
          {total === 0 && (
            <div className="empty">
              {searching ? "No messages match that search." : "This conversation has no messages in your package."}
            </div>
          )}
          <div style={{ height: v.getTotalSize(), position: "relative" }}>
            {virtualItems.map((row) => {
              const base = items[row.index];
              const m = base && (revealed.get(base.id) ?? base);
              const isRevealed = !!base && revealed.has(base.id);
              return (
                <div
                  key={row.key}
                  data-index={row.index}
                  ref={v.measureElement}
                  className={`msg${m && m.id === highlight ? " flash" : ""}`}
                  style={{ position: "absolute", top: 0, left: 0, right: 0, transform: `translateY(${row.start}px)` }}
                >
                  {m ? (
                    <>
                      <time>{dateTime(m.ts)}</time>
                      <div>
                        {searching && everywhere && <div className="msg-chan">{m.channel_name}</div>}
                        <div className="message-text">
                          {m.content === "" ? (
                            <span className="muted">(no text)</span>
                          ) : (
                            m.segments.map((seg, i) =>
                              seg.sensitive ? (
                                <button
                                  key={i}
                                  className={`redact${isRevealed ? " shown" : ""}`}
                                  onClick={() => toggleReveal(base!)}
                                  title={isRevealed ? "Click to hide" : "Click to show"}
                                  aria-label={isRevealed ? "Hide again" : "Show hidden text"}
                                >
                                  {seg.text}
                                </button>
                              ) : (
                                <span key={i}>{searching ? mark(seg.text, words) : seg.text}</span>
                              ),
                            )
                          )}
                        </div>
                        {m.attachments.map((a) => (
                          <div className="attach" key={a}>Attached file: {fileName(a)}</div>
                        ))}
                        {m.link && (
                          <button className="link small msg-open" onClick={() => openLink(m.link)}>
                            {openLabel(m.platform)}
                          </button>
                        )}
                      </div>
                    </>
                  ) : (
                    <>
                      <span />
                      <div className="placeholder" />
                    </>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      </section>
    </div>
  );
}

/** Highlights the searched words. Plain text only, never HTML. */
function mark(text: string, words: string[]): ReactNode {
  if (!words.length) return text;
  const esc = words.map((w) => w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  const parts = text.split(new RegExp(`(${esc.join("|")})`, "gi"));
  return parts.map((p, i) => (i % 2 === 1 ? <mark key={i}>{p}</mark> : p));
}
