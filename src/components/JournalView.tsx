import React, { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Search, ChevronRight, BookOpen } from "lucide-react";

// ── Types ─────────────────────────────────────────────────────────────────────

interface JournalDay {
  date_str: string;
  entry_count: number;
  word_count: number;
  first_ts: number;
  last_ts: number;
}

interface HistoryEntry {
  id: number;
  timestamp: number;
  title: string;
  transcription_text: string;
  post_processed_text: string | null;
}

interface JournalSearchResult {
  id: number;
  timestamp: number;
  title: string;
  snippet: string;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function formatTime(ts: number): string {
  return new Date(ts * 1000).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatDate(dateStr: string): string {
  // dateStr is "YYYY-MM-DD"
  const [year, month, day] = dateStr.split("-").map(Number);
  const d = new Date(year, month - 1, day);
  const today = new Date();
  const yesterday = new Date(today);
  yesterday.setDate(yesterday.getDate() - 1);

  const isToday =
    d.getFullYear() === today.getFullYear() &&
    d.getMonth() === today.getMonth() &&
    d.getDate() === today.getDate();
  const isYesterday =
    d.getFullYear() === yesterday.getFullYear() &&
    d.getMonth() === yesterday.getMonth() &&
    d.getDate() === yesterday.getDate();

  if (isToday) return "Today";
  if (isYesterday) return "Yesterday";

  return d.toLocaleDateString([], {
    weekday: "short",
    month: "short",
    day: "numeric",
    year: d.getFullYear() !== today.getFullYear() ? "numeric" : undefined,
  });
}

function wordCount(text: string): number {
  return text.trim() === "" ? 0 : text.trim().split(/\s+/).length;
}

// ── Day list panel ────────────────────────────────────────────────────────────

const DayList: React.FC<{
  days: JournalDay[];
  selectedDate: string | null;
  onSelect: (d: string) => void;
}> = ({ days, selectedDate, onSelect }) => {
  if (days.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-full gap-2 text-mid-gray/60 p-6 text-center">
        <BookOpen size={32} className="opacity-40" />
        <p className="text-sm">No transcriptions yet.</p>
        <p className="text-xs">Your journal fills automatically as you dictate.</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col overflow-y-auto h-full">
      {days.map((day) => {
        const isActive = day.date_str === selectedDate;
        return (
          <button
            key={day.date_str}
            onClick={() => onSelect(day.date_str)}
            className={`flex items-center justify-between px-3 py-2.5 text-left transition-colors hover:bg-mid-gray/10 border-b border-mid-gray/10 ${
              isActive ? "bg-logo-primary/20" : ""
            }`}
          >
            <div className="min-w-0">
              <p className={`text-sm font-medium truncate ${isActive ? "text-logo-primary" : ""}`}>
                {formatDate(day.date_str)}
              </p>
              <p className="text-xs text-mid-gray/60 mt-0.5">
                {day.entry_count} {day.entry_count === 1 ? "entry" : "entries"} ·{" "}
                {day.word_count.toLocaleString()} words
              </p>
            </div>
            <ChevronRight size={14} className="shrink-0 text-mid-gray/40" />
          </button>
        );
      })}
    </div>
  );
};

// ── Day detail panel ──────────────────────────────────────────────────────────

const DayDetail: React.FC<{
  dateStr: string;
  entries: HistoryEntry[];
  loading: boolean;
}> = ({ dateStr, entries, loading }) => {
  if (loading) {
    return (
      <div className="flex items-center justify-center h-full text-mid-gray/50 text-sm">
        Loading…
      </div>
    );
  }

  if (entries.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-mid-gray/50 text-sm">
        No entries for {formatDate(dateStr)}.
      </div>
    );
  }

  return (
    <div className="flex flex-col overflow-y-auto h-full gap-0">
      <div className="px-4 pt-3 pb-2 border-b border-mid-gray/20 shrink-0">
        <h2 className="text-base font-semibold">{formatDate(dateStr)}</h2>
        <p className="text-xs text-mid-gray/60 mt-0.5">
          {entries.length} {entries.length === 1 ? "entry" : "entries"} ·{" "}
          {entries
            .reduce((acc, e) => acc + wordCount(e.transcription_text), 0)
            .toLocaleString()}{" "}
          words
        </p>
      </div>
      <div className="flex flex-col divide-y divide-mid-gray/10 overflow-y-auto">
        {entries.map((entry) => {
          const display = entry.post_processed_text ?? entry.transcription_text;
          return (
            <div key={entry.id} className="px-4 py-3 group">
              <div className="flex items-center gap-2 mb-1">
                <span className="text-xs text-mid-gray/50 font-mono">
                  {formatTime(entry.timestamp)}
                </span>
                {entry.title && entry.title !== "Untitled" && (
                  <span className="text-xs text-mid-gray/70 truncate">
                    {entry.title}
                  </span>
                )}
              </div>
              <p className="text-sm leading-relaxed text-text/90 whitespace-pre-wrap break-words">
                {display}
              </p>
            </div>
          );
        })}
      </div>
    </div>
  );
};

// ── Search results panel ──────────────────────────────────────────────────────

const SearchResults: React.FC<{
  results: JournalSearchResult[];
  loading: boolean;
  query: string;
}> = ({ results, loading, query }) => {
  if (loading) {
    return (
      <div className="flex items-center justify-center h-full text-mid-gray/50 text-sm">
        Searching…
      </div>
    );
  }

  if (results.length === 0 && query.trim() !== "") {
    return (
      <div className="flex items-center justify-center h-full text-mid-gray/50 text-sm">
        No results for "{query}"
      </div>
    );
  }

  return (
    <div className="flex flex-col overflow-y-auto h-full divide-y divide-mid-gray/10">
      {results.map((r) => (
        <div key={r.id} className="px-4 py-3">
          <div className="flex items-center gap-2 mb-1">
            <span className="text-xs text-mid-gray/50 font-mono">
              {new Date(r.timestamp * 1000).toLocaleDateString([], {
                month: "short",
                day: "numeric",
              })}{" "}
              {formatTime(r.timestamp)}
            </span>
          </div>
          <p
            className="text-sm leading-relaxed text-text/90 whitespace-pre-wrap break-words"
            dangerouslySetInnerHTML={{
              // The ** markers from FTS snippet() are converted to <strong>
              __html: r.snippet.replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>"),
            }}
          />
        </div>
      ))}
    </div>
  );
};

// ── Main JournalView ──────────────────────────────────────────────────────────

export const JournalView: React.FC = () => {
  const [days, setDays] = useState<JournalDay[]>([]);
  const [selectedDate, setSelectedDate] = useState<string | null>(null);
  const [dayEntries, setDayEntries] = useState<HistoryEntry[]>([]);
  const [loadingDay, setLoadingDay] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<JournalSearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [loadingDays, setLoadingDays] = useState(false);

  // Load day list on mount
  useEffect(() => {
    setLoadingDays(true);
    invoke<JournalDay[]>("list_journal_days")
      .then((d) => {
        setDays(d ?? []);
        if (d && d.length > 0) {
          setSelectedDate(d[0].date_str);
        }
      })
      .catch(console.error)
      .finally(() => setLoadingDays(false));
  }, []);

  // Load entries when selected date changes
  useEffect(() => {
    if (!selectedDate) return;
    setLoadingDay(true);
    invoke<HistoryEntry[]>("get_journal_day", { dateStr: selectedDate })
      .then((e) => setDayEntries(e ?? []))
      .catch(console.error)
      .finally(() => setLoadingDay(false));
  }, [selectedDate]);

  // Debounced search
  useEffect(() => {
    const trimmed = searchQuery.trim();
    if (trimmed === "") {
      setSearchResults([]);
      return;
    }

    const timer = setTimeout(async () => {
      setSearching(true);
      try {
        const results = await invoke<JournalSearchResult[]>("search_journal", {
          query: trimmed,
          limit: 50,
        });
        setSearchResults(results ?? []);
      } catch (e) {
        console.error("search_journal:", e);
      } finally {
        setSearching(false);
      }
    }, 300);

    return () => clearTimeout(timer);
  }, [searchQuery]);

  const isSearching = searchQuery.trim() !== "";

  return (
    <div className="flex h-full w-full overflow-hidden">
      {/* Left column: day list */}
      <div className="w-48 shrink-0 border-e border-mid-gray/20 flex flex-col h-full overflow-hidden">
        {/* Search box */}
        <div className="px-2 py-2 border-b border-mid-gray/20 shrink-0">
          <div className="flex items-center gap-1.5 bg-mid-gray/10 rounded-md px-2 py-1.5">
            <Search size={13} className="text-mid-gray/50 shrink-0" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search…"
              className="bg-transparent text-sm outline-none w-full text-text placeholder:text-mid-gray/40"
            />
          </div>
        </div>

        {/* Day list */}
        <div className="flex-1 overflow-hidden">
          {loadingDays ? (
            <div className="flex items-center justify-center h-full text-mid-gray/50 text-xs">
              Loading…
            </div>
          ) : (
            <DayList
              days={days}
              selectedDate={selectedDate}
              onSelect={(d) => {
                setSelectedDate(d);
                setSearchQuery("");
              }}
            />
          )}
        </div>
      </div>

      {/* Right column: detail / search results */}
      <div className="flex-1 overflow-hidden">
        {isSearching ? (
          <SearchResults
            results={searchResults}
            loading={searching}
            query={searchQuery}
          />
        ) : selectedDate ? (
          <DayDetail
            dateStr={selectedDate}
            entries={dayEntries}
            loading={loadingDay}
          />
        ) : (
          <div className="flex items-center justify-center h-full text-mid-gray/50 text-sm">
            Select a day to view entries.
          </div>
        )}
      </div>
    </div>
  );
};

export default JournalView;
